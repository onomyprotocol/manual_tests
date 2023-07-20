use std::time::Duration;

use common::container_runner;
use onomy_test_lib::{
    cosmovisor::{cosmovisor_get_addr, cosmovisor_start, sh_cosmovisor_no_dbg},
    dockerfiles::onomy_std_cosmos_daemon,
    onomy_std_init,
    setups::market_standalone_setup,
    super_orchestrator::{
        sh,
        stacked_errors::{MapAddError, Result},
        Command, FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

const CHAIN_ID: &str = "appname";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "standalone" => standalone_runner(&args).await,
            _ => format!("entry_name \"{s}\" is not recognized").map_add_err(|| ()),
        }
    } else {
        let mut cmd = Command::new("go build ./cmd/standalone", &[]).ci_mode(true);
        cmd.cwd = Some("./../multiverse/".to_owned());
        let comres = cmd.run_to_completion().await?;
        comres.assert_success()?;
        sh("cd ./../manual_test/", &[]).await?;
        // copy to dockerfile resources (docker cannot use files from outside cwd)
        sh(
            &format!(
                "cp ./../multiverse/standalone \
                 ./tests/dockerfiles/dockerfile_resources/{CHAIN_ID}d"
            ),
            &[],
        )
        .await?;
        container_runner(&args, &[(
            "standalone",
            &onomy_std_cosmos_daemon(
                &format!("{CHAIN_ID}d"),
                &format!(".onomy_{CHAIN_ID}"),
                "v0.1.0",
                &format!("{CHAIN_ID}d"),
            ),
        )])
        .await
    }
}

async fn standalone_runner(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().map_add_err(|| ())?;
    market_standalone_setup(daemon_home, CHAIN_ID).await?;
    let mut cosmovisor_runner = cosmovisor_start(&format!("{CHAIN_ID}d_runner.log"), None).await?;

    let addr = &cosmovisor_get_addr("validator").await?;
    sh(
        &format!(
            "cosmovisor run tx bank send {addr} onomy1a69w3hfjqere4crkgyee79x2mxq0w2pfj9tu2m \
             1337anative --gas auto --gas-adjustment 1.3 -y -b block"
        ),
        &[],
    )
    .await?;

    sleep(Duration::ZERO).await;
    cosmovisor_runner.terminate(TIMEOUT).await?;
    // test that exporting works
    let exported = sh_cosmovisor_no_dbg("export", &[]).await?;
    FileOptions::write_str(&format!("/logs/{CHAIN_ID}d_export.json"), &exported).await?;

    Ok(())
}
