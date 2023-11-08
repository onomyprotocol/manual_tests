use common::dockerfile_onexd;
use onomy_test_lib::{
    cosmovisor::{
        cosmovisor_start, set_persistent_peers, sh_cosmovisor, wait_for_num_blocks,
        CosmovisorOptions,
    },
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
        FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

// note: if you ever get an error like "auth failure: secret conn failed: proto:
// BytesValue: wiretype end group for non-group", it is likely because P2P is
// going to the GRPC port instead of the correct one which is usually on port
// 26656
const GENESIS: &str =
    include_str!("./../../../../environments/testnet/onex-testnet-3/genesis.json");
const PEER_INFO: &str = "e7ea2a55be91e35f5cf41febb60d903ed2d07fea@34.86.135.162:26656";
const CHAIN_ID: &str = "onex-testnet-3";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onexd" => onexd_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args).await.stack()
    }
}

pub async fn container_runner(args: &Args) -> Result<()> {
    let logs_dir = "./tests/logs";
    let resources_dir = "./tests/resources";
    let dockerfiles_dir = "./tests/dockerfiles";
    let bin_entrypoint = &args.bin_name;
    let container_target = "x86_64-unknown-linux-gnu";

    // build internal runner
    sh([
        "cargo build --release --bin",
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await
    .stack()?;

    let mut cn = ContainerNetwork::new(
        "test",
        vec![
            Container::new("onexd", Dockerfile::Contents(dockerfile_onexd())).entrypoint(
                format!("./target/{container_target}/release/{bin_entrypoint}"),
                ["--entry-name", "onexd"],
            ),
        ],
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )
    .stack()?;
    cn.add_common_volumes([(logs_dir, "/logs"), (resources_dir, "/resources")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(["--uuid", &uuid]);
    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn onexd_runner(args: &Args) -> Result<()> {
    //sleep(TIMEOUT).await;
    let daemon_home = args.daemon_home.clone().stack()?;

    //sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor(["config chain-id", CHAIN_ID]).await.stack()?;
    sh_cosmovisor(["config keyring-backend test"])
        .await
        .stack()?;

    FileOptions::write_str(&format!("{daemon_home}/config/genesis.json"), GENESIS)
        .await
        .stack()?;
    set_persistent_peers(&daemon_home, &[PEER_INFO.to_owned()])
        .await
        .stack()?;

    let mut options = CosmovisorOptions::new();
    options.wait_for_status_only = true;
    let mut cosmos_runner = cosmovisor_start("/logs/full_node.log", Some(options))
        .await
        .stack()?;

    sh_cosmovisor(["query block"]).await.stack()?;
    wait_for_num_blocks(1).await.stack()?;
    sh_cosmovisor(["query ccvconsumer next-fee-distribution"])
        .await
        .stack()?;
    sh_cosmovisor(["query slashing signing-infos"])
        .await
        .stack()?;

    sleep(TIMEOUT).await;
    cosmos_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
