use onomy_test_lib::{
    dockerfiles::dockerfile_hermes,
    hermes::{hermes_start, sh_hermes, write_hermes_config, HermesChainConfig},
    ibc::IbcPair,
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

const ONOMY_NODE: &str = "http://34.28.227.180:26657";
const CONSUMER_NODE: &str = "";
const ONOMY_CHAIN_ID: &str = "onomy-testnet-1";
const CONSUMER_CHAIN_ID: &str = "onex-testnet-1";
const PROVIDER_MNEMONIC: &str = include_str!("./../../../../testnet_provider_mnemonic.txt");
const DEALER_MNEMONIC: &str = include_str!("./../../../../testnet_dealer_mnemonic.txt");

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "hermes" => hermes_runner(&args).await,
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

    let mut cn = ContainerNetwork::new(
        "test",
        vec![Container::new(
            "hermes",
            Dockerfile::Contents(dockerfile_hermes("__tmp_hermes_config.toml")),
            entrypoint,
            &["--entry-name", "hermes"],
        )],
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )
    .stack()?;
    cn.add_common_volumes(&[(logs_dir, "/logs")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(&["--uuid", &uuid]);

    // prepare hermes config
    write_hermes_config(
        &[
            HermesChainConfig::new(ONOMY_CHAIN_ID, ONOMY_NODE, "onomy", false, "anom", false),
            HermesChainConfig::new(
                CONSUMER_CHAIN_ID,
                CONSUMER_NODE,
                "onomy",
                true,
                "anom",
                false,
            ),
        ],
        &format!("{dockerfiles_dir}/dockerfile_resources"),
    )
    .await
    .stack()?;

    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn hermes_runner(_args: &Args) -> Result<()> {
    let provider_mnemonic = PROVIDER_MNEMONIC;
    let dealer_mnemonic = DEALER_MNEMONIC;

    // set keys for our chains
    FileOptions::write_str("/root/.hermes/provider_mnemonic.txt", provider_mnemonic)
        .await
        .stack()?;
    FileOptions::write_str("/root/.hermes/dealer_mnemonic.txt", dealer_mnemonic)
        .await
        .stack()?;

    sh_hermes(
        &format!(
            "keys add --chain {ONOMY_CHAIN_ID} --mnemonic-file /root/.hermes/provider_mnemonic.txt"
        ),
        &[],
    )
    .await
    .stack()?;
    sh_hermes(
        &format!(
            "keys add --chain {CONSUMER_CHAIN_ID} --mnemonic-file \
             /root/.hermes/dealer_mnemonic.txt"
        ),
        &[],
    )
    .await
    .stack()?;

    // this needs to be done just once
    let ibc_pair = IbcPair::hermes_setup_ics_pair(CONSUMER_CHAIN_ID, ONOMY_CHAIN_ID).await?;

    // then we need to relay
    let mut hermes_runner = hermes_start("/logs/hermes_bootstrap_runner.log").await?;
    ibc_pair.hermes_check_acks().await.stack()?;

    sleep(TIMEOUT).await;

    hermes_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
