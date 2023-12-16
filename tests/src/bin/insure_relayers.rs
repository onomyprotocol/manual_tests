//! Run this manually once a week to insure that ICS channels do not expire
//!
//! Check the outputs in the cosole and in ./tests/logs/hermes_ics_runner.log to
//! make sure there are no errors or significant warnings

/*
e.x.

cargo r --bin insure_relayers -- --mnemonic-path ./../testnet_dealer_mnemonic.txt

// run this to be able to terminate or run `hermes` in the container
cargo r --bin auto_exec_i -- --container-name hermes

*/

use lazy_static::lazy_static;
use log::warn;
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

lazy_static! {
    // A list of chains and their corresponding ICS clients that should be updated.
    // The provider chain has a client for every consumer, and the consumer chains
    // have one client with their provider.
    static ref HERMES_CONFIGS: Vec<(HermesChainConfig, Vec<String>)> = vec![
        // mainnet TODO

        // testnet
        (
            HermesChainConfig::new(
                "onomy-testnet-1",
                "34.145.158.212",
                "onomy",
                false,
                "anom",
                false,
            ),
            vec!["07-tendermint-10".to_owned()],
        ),
        (
            HermesChainConfig::new(
                "onex-testnet-4",
                "34.86.135.162",
                "onomy",
                true,
                "abtc",
                false,
            ),
            vec!["07-tendermint-0".to_owned()],
        ),

        // Note: in case you are running multiple nodes on the same machine and are using
        // different port numbers from the default, you can change them like this
        /*{
            let hostname = "34.86.135.162";
            let mut config = HermesChainConfig::new(
                "onex-testnet-3",
                hostname,
                "onomy",
                true,
                "anom",
                false,
            );
            config.rpc_addr = format!("http://{hostname}:36657");
            config.grpc_addr = format!("http://{hostname}:9292");
            config.event_addr = format!("ws://{hostname}:36657/websocket");

            (config, vec!["07-tendermint-0".to_owned()])
        },*/
    ];
}

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

    // prepare hermes config
    write_hermes_config(
        &HERMES_CONFIGS
            .iter()
            .map(|config_and_clients| config_and_clients.0.clone())
            .collect::<Vec<HermesChainConfig>>(),
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

    // add the chains
    for (config, _) in HERMES_CONFIGS.iter() {
        sh_hermes([format!(
            "keys add --chain {} --mnemonic-file /root/.hermes/dealer_mnemonic.txt",
            config.chain_id
        )])
        .await
        .stack()?;
    }

    // update clients once, insures clients are updated even with no packets being
    // relayed
    for (config, clients) in HERMES_CONFIGS.iter() {
        for client in clients {
            let res = sh_hermes([format!(
                "update client --host-chain {} --client {}",
                config.chain_id, client
            )])
            .await
            .stack();
            if res.is_err() {
                warn!("updating client failed with {:?}", res);
            }
        }
    }

    let mut hermes_runner = hermes_start("/logs/hermes_ics_runner.log").await.stack()?;
    sleep(TIMEOUT).await;
    hermes_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
