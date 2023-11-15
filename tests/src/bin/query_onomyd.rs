//! Query onomyd, use auto_exec_i to get into the container and issue
//! `cosmovisor` commands

use onomy_test_lib::dockerfiles::dockerfile_onomyd;
#[rustfmt::skip]
/*
e.x.

cargo r --bin query_onomyd -- --mnemonic-path ./../testnet_dealer_mnemonic.txt --node http://34.145.158.212:26657

// in another terminal
cargo r --bin auto_exec_i -- --container-name onomyd

*/

use onomy_test_lib::{
    cosmovisor::sh_cosmovisor,
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

const CHAIN_ID: &str = "onomy-testnet-1";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onomyd" => onomyd_runner(&args).await,
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
                .stack_err(|| "need --mnemonic-path")?,
            "./tests/resources/tmp/mnemonic.txt",
        )
        .await
        .stack()?;

        let mut containers = vec![];
        containers.push(
            Container::new("onomyd", Dockerfile::contents(dockerfile_onomyd()))
                .external_entrypoint(
                    format!("./target/{container_target}/release/{bin_entrypoint}"),
                    [
                        "--entry-name",
                        "onomyd",
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

async fn onomyd_runner(args: &Args) -> Result<()> {
    // http://34.145.158.212:26657/validators?
    // http://34.145.158.212:1317/

    let daemon_home = args.daemon_home.as_ref().stack()?;

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
    sh_cosmovisor(["query slashing signing-infos"])
        .await
        .stack()?;

    Command::new(format!(
        "{daemon_home}/cosmovisor/current/bin/onomyd keys add validator --recover"
    ))
    .run_with_input_to_completion(mnemonic.as_bytes())
    .await
    .stack()?
    .assert_success()
    .stack()?;

    sleep(TIMEOUT).await;

    //cosmovisor run tx bank send validator
    // onomy1tmtdfh2wm343nkk4424jqe9n0j0ecw870qd9c2 1000000000000000000000anom -y -b
    // block --from validator

    //100000000000000000000000000
    //     1000000000000000000000anom

    //onomy1yks83spz6lvrrys8kh0untt22399tskk6jafcv

    Ok(())
}
