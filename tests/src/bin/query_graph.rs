#![allow(unused)]

use std::time::Duration;

use common::{DOWNLOAD_ONEXD, ONEXD_FH_VERSION};
use log::info;
use onomy_test_lib::{
    cosmovisor::{set_persistent_peers, sh_cosmovisor, sh_cosmovisor_no_dbg},
    dockerfiles::{COSMOVISOR, ONOMY_STD},
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
        wait_for_ok, Command, FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

// NOTE: this binary stores stuff in /resources/query_graph. There is some code
// that should be uncommented on the first run to initialize it

// NOTE: the `common-first-streamable-block` flag may need to be changed
// depending on pruning settings

// we use a normal onexd for the validator full node, but use the `-fh` version
// for the full node that indexes for firehose

const GENESIS: &str =
    include_str!("./../../../../environments/testnet/onex-testnet-3/genesis.json");
const PEER_INFO: &str = "e7ea2a55be91e35f5cf41febb60d903ed2d07fea@34.86.135.162:26656";
const CHAIN_ID: &str = "onex-testnet-3";
const BINARY_NAME: &str = "onexd";
const BINARY_DIR: &str = ".onomy_onex";
const FIREHOSE_CONFIG_PATH: &str = "/firehose/firehose.yml";
const FIREHOSE_CONFIG: &str = r#"start:
    args:
        - reader
        - relayer
        - merger
        - firehose
    flags:
        common-first-streamable-block: 170700
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
RUN git clone https://github.com/onomyprotocol/mgraph
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
    sh("cargo build --release --bin", &[
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await
    .stack()?;

    let entrypoint = Some(format!(
        "./target/{container_target}/release/{bin_entrypoint}"
    ));
    let entrypoint = entrypoint.as_deref();

    // we use a normal onexd for the validator full node, but use the `-fh` version
    // for the full node that indexes for firehose
    let containers = vec![Container::new(
        "test_runner",
        Dockerfile::Contents(standalone_dockerfile()),
        entrypoint,
        &["--entry-name", "test_runner"],
    )
    .volumes(&[("./tests/resources/query_graph", "/firehose")])];

    let mut cn =
        ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir).stack()?;
    cn.add_common_volumes(&[(logs_dir, "/logs")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(&["--uuid", &uuid]);
    cn.add_container(
        Container::new(
            "postgres",
            Dockerfile::NameTag("postgres:16".to_owned()),
            None,
            &[],
        )
        .environment_vars(&[
            ("POSTGRES_PASSWORD", "root"),
            ("POSTGRES_USER", "postgres"),
            ("POSTGRES_DB", "graph-node"),
            ("POSTGRES_INITDB_ARGS", "-E UTF8 --locale=C"),
        ])
        .no_uuid_for_host_name(),
    )
    .stack()?;

    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, Duration::from_secs(9999))
        .await
        .stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn test_runner(args: &Args) -> Result<()> {
    let uuid = &args.uuid;
    let firehose_err_log = FileOptions::write2("/logs", "firehose_err.log");
    let firehose_std_log = FileOptions::write2("/logs", "firehose_std.log");
    let ipfs_log = FileOptions::write2("/logs", "ipfs.log");
    let graph_log = FileOptions::write2("/logs", "graph.log");

    let mut ipfs_runner = Command::new("ipfs daemon", &[])
        .stderr_log(&ipfs_log)
        .stdout_log(&ipfs_log)
        .run()
        .await
        .stack()?;

    FileOptions::write_str(GRAPH_NODE_CONFIG_PATH, GRAPH_NODE_CONFIG)
        .await
        .stack()?;

    FileOptions::write_str(FIREHOSE_CONFIG_PATH, FIREHOSE_CONFIG)
        .await
        .stack()?;

    async fn postgres_health(uuid: &str) -> Result<()> {
        let comres = Command::new(
            &format!("psql --host=postgres_{uuid} -U postgres --command=\\l"),
            &[],
        )
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

    // uncomment for first run
    /*
    sh_cosmovisor("config chain-id --home /firehose", &[CHAIN_ID])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test --home /firehose", &[])
        .await
        .stack()?;
    sh_cosmovisor_no_dbg("init --overwrite --home /firehose", &[CHAIN_ID])
        .await
        .stack()?;
    FileOptions::write_str("/firehose/config/genesis.json", GENESIS)
        .await
        .stack()?;
    set_persistent_peers("/firehose", &[PEER_INFO.to_owned()])
        .await
        .stack()?;
    let mut config = FileOptions::read_to_string(CONFIG_TOML_PATH)
        .await
        .stack()?;
    config.push_str(EXTRACTOR_CONFIG);
    FileOptions::write_str(CONFIG_TOML_PATH, &config)
        .await
        .stack()?;
    */

    let mut firecosmos_runner = Command::new(
        "firecosmos start --config /firehose/firehose.yml --data-dir /firehose/fh-data \
         --firehose-grpc-listen-addr 0.0.0.0:9030",
        &[],
    )
    .stderr_log(&firehose_err_log)
    .stdout_log(&firehose_std_log)
    .run()
    .await
    .stack()?;

    // should see stuff from
    //grpcurl -plaintext -max-time 1 localhost:9030 sf.firehose.v2.Stream/Blocks

    //sleep(Duration::from_secs(9999)).await;

    async fn firecosmos_health() -> Result<()> {
        let comres = Command::new("curl -sL -w 200 http://localhost:9030 -o /dev/null", &[])
            .run_to_completion()
            .await
            .stack()?;
        comres.assert_success().stack()?;
        Ok(())
    }
    wait_for_ok(100, Duration::from_secs(1), firecosmos_health)
        .await
        .stack()?;
    info!("firehose is up");

    let mut graph_runner = Command::new(
        &format!(
            "cargo run --release -p graph-node -- --config {GRAPH_NODE_CONFIG_PATH} --ipfs \
             127.0.0.1:5001 --node-id index_node_cosmos_1"
        ),
        &[],
    )
    .cwd("/graph-node")
    .stderr_log(&graph_log)
    .stdout_log(&graph_log)
    .run()
    .await
    .stack()?;

    async fn graph_node_health() -> Result<()> {
        let comres = Command::new("curl -sL -w 200 http://localhost:8020 -o /dev/null", &[])
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

    let comres = Command::new("npm run create-local", &[])
        .cwd("/mgraph")
        .ci_mode(true)
        .run_to_completion()
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let comres = Command::new(
        "graph deploy --version-label v0.0.0 --node http://localhost:8020/ \
        --ipfs http://localhost:5001 onomyprotocol/mgraph",
        &[],
    )
    .cwd("/mgraph")
    .ci_mode(true)
    .run_to_completion()
    .await
    .stack()?;
    comres.assert_success().stack()?;

    // grpcurl -plaintext -max-time 2 localhost:9030 sf.firehose.v2.Stream/Blocks
    // note: we may need to pass the proto files, I don't know if reflection is not
    // working and that's why it has errors

    sleep(Duration::from_secs(9999)).await;

    sleep(Duration::ZERO).await;
    graph_runner.terminate().await.stack()?;
    firecosmos_runner.terminate().await.stack()?;
    ipfs_runner.terminate().await.stack()?;

    Ok(())
}
