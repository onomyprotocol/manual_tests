use std::time::Duration;

use common::dockerfile_onomyd;
use onomy_test_lib::{
    cosmovisor::cosmovisor_start,
    dockerfiles::dockerfile_hermes,
    hermes::{
        create_client_pair, create_connection_pair, hermes_start, sh_hermes, write_hermes_config,
        HermesChainConfig,
    },
    onomy_std_init,
    setups::{onomyd_setup, CosmosSetupOptions},
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        net_message::NetMessenger,
        remove_files_in_dir, sh,
        stacked_errors::{Error, Result, StackableErr},
        FileOptions, STD_DELAY, STD_TRIES,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

// NOTE: this test needs to have the `spawn_time` and `genesis_time` set to a
// little before the date that this test is run, otherwise the consumer will not
// start on time or the test will not be able to query some things

const CONSUMER_ID: &str = "onex-testnet-1";
const CONSUMER_HOSTNAME: &str = "consumer";
const CONSUMER_ACCOUNT_PREFIX: &str = "onomy";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onomyd" => onomyd_runner(&args).await,
            "consumer" => consumer(&args).await,
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

    // prepare volumed resources
    remove_files_in_dir("./tests/resources/keyring-test/", &[".address", ".info"])
        .await
        .stack()?;

    // prepare hermes config
    write_hermes_config(
        &[
            HermesChainConfig::new("onomy", "onomyd", "onomy", false, "anom", true),
            HermesChainConfig::new(
                CONSUMER_ID,
                CONSUMER_HOSTNAME,
                CONSUMER_ACCOUNT_PREFIX,
                false,
                "anom",
                true,
            ),
        ],
        &format!("{dockerfiles_dir}/dockerfile_resources"),
    )
    .await
    .stack()?;

    let entrypoint = Some(format!(
        "./target/{container_target}/release/{bin_entrypoint}"
    ));
    let entrypoint = entrypoint.as_deref();

    let mut cn = ContainerNetwork::new(
        "test",
        vec![
            Container::new(
                "hermes",
                Dockerfile::Contents(dockerfile_hermes("__tmp_hermes_config.toml")),
                entrypoint,
                &["--entry-name", "hermes"],
            ),
            Container::new(
                "onomyd",
                Dockerfile::Contents(dockerfile_onomyd()),
                entrypoint,
                &["--entry-name", "onomyd"],
            )
            .volumes(&[
                (
                    "./tests/resources/keyring-test",
                    "/root/.onomy/keyring-test",
                ),
                ("./tests/resources/", "/resources/"),
            ]),
            Container::new(
                CONSUMER_HOSTNAME,
                Dockerfile::Contents(dockerfile_onomyd()),
                entrypoint,
                &["--entry-name", "consumer"],
            )
            .volumes(&[
                (
                    "./tests/resources/keyring-test",
                    "/root/.onomy_onex/keyring-test",
                ),
                ("./tests/resources/", "/resources/"),
            ]),
        ],
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )
    .stack()?
    .add_common_volumes(&[(logs_dir, "/logs")]);
    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    Ok(())
}

async fn hermes_runner(_args: &Args) -> Result<()> {
    let mut nm_onomyd = NetMessenger::listen_single_connect("0.0.0.0:26000", TIMEOUT)
        .await
        .stack()?;

    // get mnemonic from onomyd
    let mnemonic: String = nm_onomyd.recv().await.stack()?;
    // set keys for our chains
    FileOptions::write_str("/root/.hermes/mnemonic.txt", &mnemonic)
        .await
        .stack()?;
    sh_hermes(
        "keys add --chain onomy --mnemonic-file /root/.hermes/mnemonic.txt",
        &[],
    )
    .await
    .stack()?;
    sh_hermes(
        &format!("keys add --chain {CONSUMER_ID} --mnemonic-file /root/.hermes/mnemonic.txt"),
        &[],
    )
    .await
    .stack()?;

    // wait for setup
    nm_onomyd.recv::<()>().await.stack()?;

    let _ = create_client_pair(CONSUMER_ID, "onomy").await.stack()?;
    let _ = create_connection_pair(CONSUMER_ID, "onomy").await.stack()?;
    /*let ibc_pair = IbcPair::hermes_setup_ics_pair(CONSUMER_ID, "onomy")
    .await
    .stack()?;*/
    let mut hermes_runner = hermes_start("/logs/hermes_bootstrap_runner.log")
        .await
        .stack()?;
    //ibc_pair.hermes_check_acks().await.stack()?;

    nm_onomyd.send::<()>(&()).await.stack()?;

    nm_onomyd.recv::<()>().await.stack()?;
    hermes_runner.terminate(TIMEOUT).await.stack()?;
    Ok(())
}

async fn onomyd_runner(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().stack()?;
    let mut nm_hermes = NetMessenger::connect(STD_TRIES, STD_DELAY, "hermes:26000")
        .await
        .stack()?;
    let mut nm_consumer =
        NetMessenger::connect(STD_TRIES, STD_DELAY, &format!("{CONSUMER_HOSTNAME}:26001"))
            .await
            .stack()?;

    let mnemonic = FileOptions::read_to_string("/resources/mnemonic.txt")
        .await
        .stack()?;
    let mut options = CosmosSetupOptions::new(daemon_home);
    options.mnemonic = Some(mnemonic);
    let mnemonic = onomyd_setup(options).await.stack()?;
    // send mnemonic to hermes
    nm_hermes.send::<String>(&mnemonic).await.stack()?;

    sleep(Duration::ZERO).await;

    let mut cosmovisor_runner = cosmovisor_start("onomyd_runner.log", None).await.stack()?;

    nm_hermes.send::<()>(&()).await.stack()?;
    nm_hermes.recv::<()>().await.stack()?;

    nm_consumer.send::<()>(&()).await.stack()?;
    nm_hermes.send::<()>(&()).await.stack()?;
    cosmovisor_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}

async fn consumer(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().stack()?;
    let chain_id = CONSUMER_ID;
    let mut nm_onomyd = NetMessenger::listen_single_connect("0.0.0.0:26001", TIMEOUT)
        .await
        .stack()?;

    let mnemonic = FileOptions::read_to_string("/resources/mnemonic.txt")
        .await
        .stack()?;
    let mut options = CosmosSetupOptions::new(daemon_home);
    options.chain_id = chain_id.to_owned();
    options.mnemonic = Some(mnemonic);
    let _ = onomyd_setup(options).await.stack()?;

    let mut cosmovisor_runner = cosmovisor_start("onomyd_runner.log", None).await.stack()?;

    nm_onomyd.recv::<()>().await.stack()?;
    cosmovisor_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
