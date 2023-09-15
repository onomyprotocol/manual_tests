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
const CONSUMER_NODE: &str = "34.145.158.212";
const ONOMY_CHAIN_ID: &str = "onomy-testnet-1";
const CONSUMER_CHAIN_ID: &str = "onex-testnet-1";
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

    let mut onex_hermes = HermesChainConfig::new(
        CONSUMER_CHAIN_ID,
        CONSUMER_NODE,
        "onomy",
        true,
        "anom",
        false,
    );
    onex_hermes.rpc_addr = format!("http://{CONSUMER_NODE}:36657");
    onex_hermes.grpc_addr = format!("http://{CONSUMER_NODE}:9292");
    onex_hermes.event_addr = format!("ws://{CONSUMER_NODE}:36657/websocket");

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
    let dealer_mnemonic = DEALER_MNEMONIC;

    // set keys for our chains
    FileOptions::write_str("/root/.hermes/dealer_mnemonic.txt", dealer_mnemonic)
        .await
        .stack()?;

    sh_hermes(
        &format!(
            "keys add --chain {ONOMY_CHAIN_ID} --mnemonic-file /root/.hermes/dealer_mnemonic.txt"
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

    // NOTE: if failure occurs in the middle, you will need to comment out parts
    // that have already succeeded
    /*
        // a client is already created because of the ICS setup
        //let client_pair = create_client_pair(a_chain, b_chain).await.stack()?;
        // create one client and connection pair that will be used for IBC transfer and
        // ICS communication
        let connection_pair = create_connection_pair(&CONSUMER_CHAIN_ID, &ONOMY_CHAIN_ID)
            .await
            .stack()?;

        create_channel_pair(
            &CONSUMER_CHAIN_ID,
            &connection_pair.0,
            "consumer",
            "provider",
            true,
        )
        .await
        .stack()?;

        let provider = ONOMY_CHAIN_ID;
        let consumer = CONSUMER_CHAIN_ID;
        let consumer_channel = "channel-1";
        let consumer_connection = "connection-0";
        let provider_connection = "connection-12";
        sh_hermes(
            &format!(
                "tx chan-open-try --dst-chain {provider} --src-chain {consumer} --dst-connection \
                 {provider_connection} --dst-port transfer --src-port transfer --src-channel \
                 {consumer_channel}"
            ),
            &[],
        )
        .await
        .stack()?;

        let provider_channel = "channel-4";

        sh_hermes(
            &format!(
                "tx chan-open-ack --dst-chain {consumer} --src-chain {provider} --dst-connection \
                 {consumer_connection} --dst-port transfer --src-port transfer --dst-channel \
                 {consumer_channel} --src-channel {provider_channel}"
            ),
            &[],
        )
        .await
        .stack()?;

        sh_hermes(
            &format!(
                "tx chan-open-confirm --dst-chain {provider} --src-chain {consumer} \
                --dst-connection {provider_connection} --dst-port transfer --src-port transfer \
                --dst-channel {provider_channel} --src-channel {consumer_channel}"
            ),
            &[],
        )
        .await
        .stack()?;
    */
    // then we need to relay
    let mut hermes_runner = hermes_start("/logs/hermes_ics_runner.log").await.stack()?;
    //ibc_pair.hermes_check_acks().await.stack()?;

    // hermes query packet pending --chain onomy-testnet-1 --port transfer --channel channel-4

    // CBC24F131C1128CAA18143EC2AFF01EF7170FE7957715D0DF7BEE21C6B6EE8F9

    // hermes tx ft-transfer --dst-chain onex-testnet-1 --src-chain onomy-testnet-1 --src-port transfer --src-channel channel-4 --amount 100000000000 --denom anom --timeout-height-offset 10 --timeout-seconds 60

    // 20000000 000000000000000000
    // 100000000000000000000000

    // NOTE: must use a ed25519 tendermint key
    // cosmovisor run tx staking create-validator --commission-max-change-rate 0.01 --commission-max-rate 0.10 --commission-rate 0.05 --min-self-delegation 1 --amount 100000000000ibc/5872224386C093865E42B18BDDA56BCB8CDE1E36B82B391E97697520053B0513 --from validator --pubkey '{"@type":"/cosmos.crypto.ed25519.PubKey","key":"1vMo7NN5rvX06zVmJ61KG00/KZB0H3rsmsoslRyaBds="}' -y -b block --fees 1000000anom

    sleep(TIMEOUT).await;

    hermes_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
