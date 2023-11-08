#![allow(unused)]

use std::time::Duration;

use clap::Parser;
use common::{DOWNLOAD_ONEXD, ONEXD_FH_VERSION};
use log::info;
use onomy_test_lib::{
    cosmovisor::{set_persistent_peers, set_pruning, sh_cosmovisor, sh_cosmovisor_no_debug},
    dockerfiles::{COSMOVISOR, ONOMY_STD},
    onomy_std_init,
    super_orchestrator::{
        acquire_dir_path,
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
        wait_for_ok, Command, FileOptions,
    },
    Args,
};
use tokio::time::sleep;

// NOTE: this binary persists cosmos, firehose, and postgres data in
// .../resources/query_graph.

// we use a normal onexd for the validator full node, but use the `-fh` version
// for the full node that indexes for firehose

// Pass `--peer-info ...` to select the peer for the firehose node to use.
// It should look like
// "e7ea2a55be91e35f5cf41febb60d903ed2d07fea@34.86.135.162:26656"

// when running for the first time or after `/resources/query_graph` has been
// cleaned, pass `--first-time` which will properly initialize it
// Also on `--first-time`,
// Pass `--genesis-path ...` to select a different path to a genesis (relative
// to the root of the repo),
// but afterwards you may have to cd into /resource/query_graph to manually
// change them

// the 8000 port is exposed

// NOTE: you may need to turn off pruning or change the
// `common-first-streamable-block` flag, and may need to uncomment a sleep line
// to not timeout before syncing is complete

const DEFAULT_GENESIS_PATH: &str = "./../environments/testnet/onex-testnet-3/genesis.json";
const CHAIN_ID: &str = "onex-testnet-3";
const BINARY_NAME: &str = "onexd";
const BINARY_DIR: &str = ".onomy_onex";
// time until the program ends after everything is deployed
const END_TIMEOUT: Duration = Duration::from_secs(1_000_000_000);

const FIREHOSE_CONFIG_PATH: &str = "/firehose/firehose.yml";
const FIREHOSE_CONFIG: &str = r#"start:
    args:
        - reader
        - relayer
        - merger
        - firehose
    flags:
        common-first-streamable-block: 1
        reader-mode: node
        reader-node-path: /root/.onomy_onex/cosmovisor/current/bin/onexd
        reader-node-args: start --x-crisis-skip-assert-invariants --home=/firehose
        reader-node-logs-filter: "module=(p2p|pex|consensus|x/bank|x/market)"
        relayer-max-source-latency: 99999h
        verbose: 1"#;

const CONFIG_TOML_PATH: &str = "/firehose/config/config.toml";
const EXTRACTOR_CONFIG: &str = r#"
[extractor]
enabled = true
output_file = "stdout"
"#;

const GRAPH_NODE_CONFIG_PATH: &str = "/graph_node_config.toml";
const GRAPH_NODE_CONFIG: &str = r#"[deployment]
[[deployment.rule]]
shard = "primary"
indexers = [ "index_node_cosmos_1" ]

[store]
[store.primary]
connection = "postgresql://postgres:root@postgres:5432/graph-node"
pool_size = 10

[chains]
ingestor = "block_ingestor_node"

[chains.market]
shard = "primary"
protocol = "cosmos"
provider = [
  { label = "market", details = { type = "firehose", url = "http://localhost:9030/" }},
]"#;

#[rustfmt::skip]
fn standalone_dockerfile() -> String {
    // use the fh version
    let version = ONEXD_FH_VERSION;
    let daemon_name = BINARY_NAME;
    let daemon_dir_name = BINARY_DIR;
    format!(
        r#"{ONOMY_STD}
# postgres and protobuf dependencies
RUN dnf install -y postgresql libpq-devel protobuf protobuf-compiler protobuf-devel
# for debug
RUN go install github.com/fullstorydev/grpcurl/cmd/grpcurl@latest
# for cosmovisor
{COSMOVISOR}

# interfacing with the running graph
RUN npm install -g @graphprotocol/graph-cli

# firehose
RUN git clone --depth 1 --branch v0.6.0 https://github.com/figment-networks/firehose-cosmos
# not working for me, too flaky
#RUN cd /firehose-cosmos && make install
ADD https://github.com/graphprotocol/firehose-cosmos/releases/download/v0.6.0/firecosmos_linux_amd64 /usr/bin/firecosmos
RUN chmod +x /usr/bin/firecosmos

# graph-node
RUN git clone --depth 1 --branch v0.32.0 https://github.com/graphprotocol/graph-node
RUN cd /graph-node && cargo build --release -p graph-node

# ipfs
ADD https://dist.ipfs.tech/kubo/v0.23.0/kubo_v0.23.0_linux-amd64.tar.gz /tmp/kubo.tar.gz
RUN cd /tmp && tar -xf /tmp/kubo.tar.gz && mv /tmp/kubo/ipfs /usr/bin/ipfs
RUN ipfs init

# our subgraph
RUN git clone --branch main https://github.com/onomyprotocol/mgraph
RUN cd /mgraph && git checkout 3e40b4731725e4210a23ebbba688a087b52360d6
#ADD ./dockerfile_resources/mgraph /mgraph
RUN cd /mgraph && npm install && npm run build

ENV DAEMON_NAME="{daemon_name}"
ENV DAEMON_HOME="/root/{daemon_dir_name}"
ENV DAEMON_VERSION={version}

{DOWNLOAD_ONEXD}

# for manual testing
RUN chmod +x $DAEMON_HOME/cosmovisor/genesis/$DAEMON_VERSION/bin/{daemon_name}

# set up symbolic links
RUN cosmovisor init $DAEMON_HOME/cosmovisor/genesis/$DAEMON_VERSION/bin/{daemon_name}

# some commands don't like if the data directory does not exist
RUN mkdir $DAEMON_HOME/data
"#
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "test_runner" => test_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args).await.stack()
    }
}

async fn container_runner(args: &Args) -> Result<()> {
    let logs_dir = "./tests/logs";
    let dockerfiles_dir = "./tests/dockerfiles";
    let bin_entrypoint = &args.bin_name;
    let container_target = "x86_64-unknown-linux-gnu";

    // build internal runner with `--release`
    sh([
        "cargo build --release --bin",
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await
    .stack()?;

    let entrypoint = format!("./target/{container_target}/release/{bin_entrypoint}");

    // we can't put these in source control with the .gitignore trick,
    // because postgres doesn't like it
    acquire_dir_path("./tests/resources/")
        .await
        .stack_err(|| "you need to run from the repo root");
    if acquire_dir_path("./tests/resources/query_graph")
        .await
        .is_err()
    {
        sh(["mkdir ./tests/resources/query_graph"]).await.stack()?;
    }
    if acquire_dir_path("./tests/resources/query_graph/postgres-data")
        .await
        .is_err()
    {
        sh(["mkdir ./tests/resources/query_graph/postgres-data"])
            .await
            .stack()?;
    }

    // copy it into place for the test runner to use
    let genesis =
        FileOptions::read_to_string(args.genesis_path.as_deref().unwrap_or(DEFAULT_GENESIS_PATH))
            .await
            .stack()?;
    FileOptions::write_str("./tests/resources/query_graph/__tmp_genesis.json", &genesis)
        .await
        .stack()?;

    let mut test_runner_args = vec!["--entry-name", "test_runner"];
    // pass on these args to the test runner
    if args.first_run {
        test_runner_args.push("--first-run");
    }
    if let Some(ref peer_info) = args.peer_info {
        test_runner_args.push("--peer-info");
        test_runner_args.push(peer_info);
    }

    // we use a normal onexd for the validator full node, but use the `-fh` version
    // for the full node that indexes for firehose
    let containers =
        vec![
            Container::new("test_runner", Dockerfile::contents(standalone_dockerfile()))
                .entrypoint(entrypoint, test_runner_args)
                // note that trying to add a ./tests/resources/ volume in addition to this will bork
                // the docker volume locally
                .volume("./tests/resources/query_graph", "/firehose")
                .create_args(["-p", "8000:8000"]),
        ];

    let mut cn =
        ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir).stack()?;
    cn.add_common_volumes([(logs_dir, "/logs")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(["--uuid", &uuid]);
    cn.add_container(
        Container::new("postgres", Dockerfile::name_tag("postgres:16"))
            .volume(
                "./tests/resources/query_graph/postgres-data",
                "/var/lib/postgresql/data",
            )
            .environment_vars([
                ("POSTGRES_PASSWORD", "root"),
                ("POSTGRES_USER", "postgres"),
                ("POSTGRES_DB", "graph-node"),
                ("POSTGRES_INITDB_ARGS", "-E UTF8 --locale=C"),
            ])
            .no_uuid_for_host_name(),
    )
    .stack()?;

    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, END_TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn test_runner(args: &Args) -> Result<()> {
    let uuid = &args.uuid;
    let firehose_err_log = FileOptions::write2("/logs", "firehose_err.log");
    let firehose_std_log = FileOptions::write2("/logs", "firehose_std.log");
    let ipfs_log = FileOptions::write2("/logs", "ipfs.log");
    let graph_log = FileOptions::write2("/logs", "graph.log");

    let mut ipfs_runner = Command::new("ipfs daemon")
        .log(Some(ipfs_log))
        .run()
        .await
        .stack()?;

    async fn postgres_health(uuid: &str) -> Result<()> {
        let comres = Command::new(format!(
            "psql --host=postgres_{uuid} -U postgres --command=\\l"
        ))
        .env("PGPASSWORD", "root")
        .run_to_completion()
        .await
        .stack()?;
        comres.assert_success().stack()?;
        Ok(())
    }
    wait_for_ok(10, Duration::from_secs(1), || postgres_health(uuid))
        .await
        .stack()?;

    if args.first_run {
        sh_cosmovisor(["config chain-id --home /firehose", CHAIN_ID])
            .await
            .stack()?;
        sh_cosmovisor(["config keyring-backend test --home /firehose"])
            .await
            .stack()?;
        sh_cosmovisor_no_debug(["init --overwrite --home /firehose", CHAIN_ID])
            .await
            .stack()?;
        // turn off pruning
        set_pruning("/firehose", "nothing").await.stack()?;
        FileOptions::write_str(
            "/firehose/config/genesis.json",
            &FileOptions::read_to_string("/firehose/__tmp_genesis.json")
                .await
                .stack()?,
        )
        .await
        .stack()?;
        let mut config = FileOptions::read_to_string(CONFIG_TOML_PATH)
            .await
            .stack()?;
        config.push_str(EXTRACTOR_CONFIG);
        FileOptions::write_str(CONFIG_TOML_PATH, &config)
            .await
            .stack()?;
    }

    // overwrite these every time
    set_persistent_peers("/firehose", &[args
        .peer_info
        .clone()
        .stack_err(|| "you need to set --peer-info")?])
    .await
    .stack()?;
    FileOptions::write_str(GRAPH_NODE_CONFIG_PATH, GRAPH_NODE_CONFIG)
        .await
        .stack()?;

    FileOptions::write_str(FIREHOSE_CONFIG_PATH, FIREHOSE_CONFIG)
        .await
        .stack()?;

    let mut firecosmos_runner = Command::new(
        "firecosmos start --config /firehose/firehose.yml --data-dir /firehose/fh-data \
         --firehose-grpc-listen-addr 0.0.0.0:9030",
    )
    .stderr_log(Some(firehose_err_log))
    .stdout_log(Some(firehose_std_log))
    .run()
    .await
    .stack()?;

    // should see stuff from
    //grpcurl -plaintext -max-time 1 localhost:9030 sf.firehose.v2.Stream/Blocks

    //sleep(Duration::from_secs(9999)).await;

    async fn firecosmos_health() -> Result<()> {
        let comres = Command::new("curl -sL -w 200 http://localhost:9030 -o /dev/null")
            .run_to_completion()
            .await
            .stack()?;
        comres.assert_success().stack()?;
        Ok(())
    }
    info!("waiting for firehose, check logs to make sure it is syncing");
    // note: if this is failing but it seems to be syncing according to the logs,
    // you may need to increase the number of retries
    wait_for_ok(100, Duration::from_secs(1), firecosmos_health)
        .await
        .stack()?;
    info!("firehose is up");

    let mut graph_runner = Command::new(format!(
        "cargo run --release -p graph-node -- --config {GRAPH_NODE_CONFIG_PATH} --ipfs \
         127.0.0.1:5001 --node-id index_node_cosmos_1"
    ))
    .cwd("/graph-node")
    .log(Some(graph_log))
    .run()
    .await
    .stack()?;

    async fn graph_node_health() -> Result<()> {
        let comres = Command::new("curl -sL -w 200 http://localhost:8020 -o /dev/null")
            .run_to_completion()
            .await
            .stack()?;
        comres.assert_success().stack()?;
        Ok(())
    }
    wait_for_ok(100, Duration::from_secs(1), graph_node_health)
        .await
        .stack()?;
    info!("graph-node is up");

    let comres = Command::new("npm run create-local")
        .cwd("/mgraph")
        .debug(true)
        .run_to_completion()
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let comres = Command::new(
        "graph deploy --version-label v0.0.0 --node http://localhost:8020/ \
            --ipfs http://localhost:5001 onomyprotocol/mgraph"
    )
    .cwd("/mgraph")
    .debug(true)
    .run_to_completion()
    .await
    .stack()?;
    comres.assert_success().stack()?;

    info!("subgraph deployed");
    info!("all things deployed, check for syncing");

    // grpcurl -plaintext -max-time 2 localhost:9030 sf.firehose.v2.Stream/Blocks
    // note: we may need to pass the proto files, I don't know if reflection is not
    // working and that's why it has errors

    sleep(END_TIMEOUT).await;

    sleep(Duration::ZERO).await;
    graph_runner.terminate().await.stack()?;
    firecosmos_runner.terminate().await.stack()?;
    ipfs_runner.terminate().await.stack()?;

    Ok(())
}
