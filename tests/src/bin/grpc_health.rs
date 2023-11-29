//! script used for testing the health of a grpc point
//!
//! pass `--grpc` to the grpc point

/*
e.x.
cargo r --bin test_grpc_health -- --grpc http://34.145.158.212:9191

*/

use deep_space::client::types::LatestBlock;
use onomy_test_lib::{
    dockerfiles::ONOMY_STD,
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
    },
    Args, TIMEOUT,
};

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "runner" => runner(&args).await,
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

        let mut runner_args = vec!["--entry-name", "runner"];
        // pass on these args to the test runner
        if let Some(ref grpc) = args.grpc {
            runner_args.push("--grpc");
            runner_args.push(grpc);
        }

        let mut containers = vec![];
        containers.push(
            Container::new("runner", Dockerfile::contents(ONOMY_STD))
                .external_entrypoint(
                    format!("./target/{container_target}/release/{bin_entrypoint}"),
                    runner_args,
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

async fn runner(args: &Args) -> Result<()> {
    let contact = deep_space::Contact::new(
        args.grpc.as_deref().stack_err(|| "need `--grpc`")?,
        TIMEOUT,
        "onomy",
    )
    .stack()?;
    let latest_block = contact.get_latest_block().await.stack()?;

    match latest_block {
        LatestBlock::Latest { block: _ } => Ok(()),
        LatestBlock::Syncing { block: _ } => Ok(()),
        LatestBlock::WaitingToStart => Ok(()),
    }
}
