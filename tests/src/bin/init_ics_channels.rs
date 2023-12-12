//! script for initiating ICS channels, please selectively comment and uncomment
//! things as needed because usually there is an error somewhere that needs to
//! be manually stepped through or skipped.
//!
//! This can also just be used to run a relayer locally temporarily (note that
//! it is relaying everything by default, may need to do some changes to
//! hermes.rs)
//!
//! Look at hermes_ics_runner.log for output from the runner

/*
e.x.

cargo r --bin init_ics_channels -- --mnemonic-path ./../testnet_dealer_mnemonic.txt

*/

use onomy_test_lib::{
    dockerfiles::dockerfile_hermes,
    hermes::{hermes_start, sh_hermes, write_hermes_config, HermesChainConfig},
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

const ONOMY_NODE: &str = "34.145.158.212";
const CONSUMER_NODE: &str = "34.86.135.162";
const ONOMY_CHAIN_ID: &str = "onomy-testnet-1";
const CONSUMER_CHAIN_ID: &str = "onex-testnet-4";

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
    sh([
        "cargo build --release --bin",
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await
    .stack()?;

    FileOptions::copy(
        args.mnemonic_path
            .as_deref()
            .stack_err(|| "need --mnemonic-path")?,
        "./tests/resources/tmp/mnemonic.txt",
    )
    .await
    .stack()?;

    let entrypoint = &format!("./target/{container_target}/release/{bin_entrypoint}");

    let mut cn = ContainerNetwork::new(
        "test",
        vec![Container::new(
            "hermes",
            Dockerfile::contents(dockerfile_hermes("__tmp_hermes_config.toml")),
        )
        .external_entrypoint(entrypoint, ["--entry-name", "hermes"])
        .await
        .stack()?],
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )
    .stack()?;
    cn.add_common_volumes([(logs_dir, "/logs"), ("./tests/resources/", "/resources/")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(["--uuid", &uuid]);

    let onex_hermes = HermesChainConfig::new(
        CONSUMER_CHAIN_ID,
        CONSUMER_NODE,
        "onomy",
        true,
        "abtc",
        false,
    );
    // in case the ports are changed from their defaults
    //onex_hermes.rpc_addr = format!("http://{CONSUMER_NODE}:36657");
    //onex_hermes.grpc_addr = format!("http://{CONSUMER_NODE}:9292");
    //onex_hermes.event_addr = format!("ws://{CONSUMER_NODE}:36657/websocket");

    let mut onomy_hermes =
        HermesChainConfig::new(ONOMY_CHAIN_ID, ONOMY_NODE, "onomy", false, "anom", false);
    onomy_hermes.grpc_addr = format!("http://{ONOMY_NODE}:9191");

    // prepare hermes config
    write_hermes_config(
        &[onomy_hermes, onex_hermes],
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
    let mnemonic = FileOptions::read_to_string("/resources/tmp/mnemonic.txt")
        .await
        .stack()?;

    // set keys for our chains
    FileOptions::write_str("/root/.hermes/dealer_mnemonic.txt", &mnemonic)
        .await
        .stack()?;

    sh_hermes([format!(
        "keys add --chain {ONOMY_CHAIN_ID} --mnemonic-file /root/.hermes/dealer_mnemonic.txt"
    )])
    .await
    .stack()?;
    sh_hermes([format!(
        "keys add --chain {CONSUMER_CHAIN_ID} --mnemonic-file /root/.hermes/dealer_mnemonic.txt"
    )])
    .await
    .stack()?;

    // NOTE: if failure occurs in the middle, you will need to comment out parts
    // that have already succeeded
    // a client is already created because of the ICS setup,
    // do not run this unless you are creating another kind of client
    //let client_pair = create_client_pair(a_chain, b_chain).await.stack()?;

    // NOTE: comment out the last relaying part when initiating ics channels

    // create one client and connection pair that will be used for IBC transfer and
    // ICS communication

    /*
    // to get the client id
    //cosmovisor run query provider list-consumer-chains
    let provider_client = "07-tendermint-10";
    // should always be this
    let consumer_client = "07-tendermint-0";

    let connection_pair = onomy_test_lib::hermes::create_connection_pair(
        &CONSUMER_CHAIN_ID,
        consumer_client,
        provider_client,
    )
    .await
    .stack()?;

    onomy_test_lib::hermes::create_channel_pair(
        &CONSUMER_CHAIN_ID,
        &connection_pair.0,
        "consumer",
        "provider",
        true,
    )
    .await
    .stack()?;
    */
    /*
    // these should always be the same
    let provider = ONOMY_CHAIN_ID;
    let consumer = CONSUMER_CHAIN_ID;
    let consumer_channel = "channel-1";
    let consumer_connection = "connection-0";
    */

    // after running the above, figure out which provider connection is needed, the
    // result should have it or you could query the hermes binary interactively

    /*
    let provider_connection = "connection-15";
    */
    /*
    sh_hermes([format!(
        "tx chan-open-try --dst-chain {provider} --src-chain {consumer} --dst-connection \
         {provider_connection} --dst-port transfer --src-port transfer --src-channel \
         {consumer_channel}"
    )])
    .await
    .stack()?;
    */

    /*
    // get this from the above op
    let provider_channel = "channel-10";

    sh_hermes([format!(
        "tx chan-open-ack --dst-chain {consumer} --src-chain {provider} --dst-connection \
         {consumer_connection} --dst-port transfer --src-port transfer --dst-channel \
         {consumer_channel} --src-channel {provider_channel}"
    )])
    .await
    .stack()?;

    sh_hermes([format!(
        "tx chan-open-confirm --dst-chain {provider} --src-chain {consumer} --dst-connection \
         {provider_connection} --dst-port transfer --src-port transfer --dst-channel \
         {provider_channel} --src-channel {consumer_channel}"
    )])
    .await
    .stack()?;
    */

    // then we need to relay
    let mut hermes_runner = hermes_start("/logs/hermes_ics_runner.log").await.stack()?;
    sleep(TIMEOUT).await;
    hermes_runner.terminate(TIMEOUT).await.stack()?;

    // hermes query packet pending --chain onomy-testnet-1 --port transfer --channel
    // channel-4

    // hermes tx ft-transfer --dst-chain onex-testnet-1 --src-chain onomy-testnet-1
    // --src-port transfer --src-channel channel-4 --amount 100000000000 --denom
    // anom --timeout-height-offset 10 --timeout-seconds 60

    Ok(())
}
