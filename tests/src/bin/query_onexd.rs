//! Query onexd, use auto_exec_i to get into the container and issue
//! `cosmovisor` commands

#[rustfmt::skip]
/*
e.x.

cargo r --bin query_onexd -- --mnemonic-path ./../testnet_dealer_mnemonic.txt --node http://34.86.135.162:26657

// in another terminal
cargo r --bin auto_exec_i -- --container-name onexd

*/

use common::dockerfile_onexd;
use onomy_test_lib::{
    cosmovisor::{sh_cosmovisor, wait_for_num_blocks},
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
        Command, FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

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

        FileOptions::copy(
            args.mnemonic_path
                .as_deref()
                .stack_err(|| "need --mnemonic")?,
            "./tests/resources/tmp/mnemonic.txt",
        )
        .await
        .stack()?;

        let mut containers = vec![];
        containers.push(
            Container::new("onexd", Dockerfile::contents(dockerfile_onexd()))
                .external_entrypoint(
                    format!("./target/{container_target}/release/{bin_entrypoint}"),
                    [
                        "--entry-name",
                        "onexd",
                        "--node",
                        args.node.as_deref().stack()?,
                    ],
                )
                .await
                .stack()?,
        );

        let mut cn =
            ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir)
                .stack()?;
        cn.add_common_volumes([(logs_dir, "/logs"), (resources_dir, "/resources")]);
        let uuid = cn.uuid_as_string();
        cn.add_common_entrypoint_args(["--uuid", &uuid]);
        cn.run_all(true).await.stack()?;
        cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
        cn.terminate_all().await;
        Ok(())
    }
}

async fn onexd_runner(args: &Args) -> Result<()> {
    // curl -s http://180.131.222.73:26756/consensus_state
    // /net_info
    // /validators

    // http://34.86.135.162:26657/validators?

    // in order to access the 1317 port locally, use `docker inspect` to find the IP
    // address of the container from the host, or use the info from auto_exec_i
    // http://34.86.135.162:1317/
    // may need to use
    //enable_swagger_apis(daemon_home).await.stack()?;
    // but note it may take over a minute to start up

    let daemon_home = args.daemon_home.clone().stack()?;

    let mnemonic = FileOptions::read_to_string("/resources/tmp/mnemonic.txt")
        .await
        .stack()?;

    sh_cosmovisor(["config node", args.node.as_deref().stack()?])
        .await
        .stack()?;
    sh_cosmovisor(["config chain-id", CHAIN_ID]).await.stack()?;
    sh_cosmovisor(["config keyring-backend test"])
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

    Command::new(format!(
        "{daemon_home}/cosmovisor/current/bin/onexd keys add validator --recover"
    ))
    .run_with_input_to_completion(mnemonic.as_bytes())
    .await
    .stack()?
    .assert_success()
    .stack()?;

    //cosmovisor run tx bank send validator
    // onomy1ll7pqzg9zscytvj9dmkl3kna50k0fundct62s7 1anom -y -b block --from
    // validator

    // ausdc,ausdt

    //cosmovisor run tx market create-order ausdc ausdt limit 1000000 1000,1000 0
    // 23 --fees 1000000anom -y -b block --from validator

    sleep(TIMEOUT).await;

    Ok(())
}
