use common::{container_runner, dockerfile_onexd};
use onomy_test_lib::{
    cosmovisor::sh_cosmovisor,
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        Command,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

const NODE: &str = "http://34.145.158.212:36657";
const CHAIN_ID: &str = "onex-testnet-1";
const MNEMONIC: &str = include_str!("./../../../../testnet_dealer_mnemonic.txt");

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

async fn onexd_runner(args: &Args) -> Result<()> {
    // curl -s http://34.145.158.212:36657/consensus_state
    // /net_info
    // /validators

    // http://34.85.152.11:36657/validators?

    let daemon_home = args.daemon_home.clone().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test", &[])
        .await
        .stack()?;

    let comres = Command::new(
        &format!("{daemon_home}/cosmovisor/current/bin/onexd keys add validator --recover"),
        &[],
    )
    .run_with_input_to_completion(MNEMONIC.as_bytes())
    .await
    .stack()?;
    comres.assert_success().stack()?;

    // cosmovisor run query ibc-transfer denom-traces

    // cosmovisor run query bank balances
    // onomy1yks83spz6lvrrys8kh0untt22399tskk6jafcv

    sleep(TIMEOUT).await;

    Ok(())
}
