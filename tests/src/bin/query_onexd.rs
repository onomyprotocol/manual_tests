use common::{container_runner, dockerfile_onexd};
use onomy_test_lib::{
    cosmovisor::sh_cosmovisor,
    onomy_std_init,
    super_orchestrator::stacked_errors::{Error, Result, StackableErr},
    Args, TIMEOUT,
};
use tokio::time::sleep;

const NODE: &str = "http://34.145.158.212:36657";
const CHAIN_ID: &str = "onex-testnet-1";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onexd" => onexd_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args, &[("onexd", &dockerfile_onexd())])
            .await
            .stack()
    }
}

async fn onexd_runner(_args: &Args) -> Result<()> {
    // curl -s http://34.145.158.212:36657/consensus_state

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;

    sleep(TIMEOUT).await;

    Ok(())
}
