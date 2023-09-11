use common::{container_runner, dockerfile_onomyd};
use onomy_test_lib::{
    cosmovisor::sh_cosmovisor,
    onomy_std_init,
    super_orchestrator::stacked_errors::{Error, Result, StackableErr},
    Args, TIMEOUT,
};
use tokio::time::sleep;

// some testnet node
//const NODE: &str = "http://34.134.208.167:26657";
const NODE: &str = "http://34.145.158.212:26657";
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
        container_runner(&args, &[("onomyd", &dockerfile_onomyd())])
            .await
            .stack()
    }
}

async fn onomyd_runner(_args: &Args) -> Result<()> {
    //let daemon_home = args.daemon_home.as_ref().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;

    sleep(TIMEOUT).await;

    Ok(())
}
