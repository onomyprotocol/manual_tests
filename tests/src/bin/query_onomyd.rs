use common::{container_runner, dockerfile_onomyd};
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

// some testnet node
//const NODE: &str = "http://34.134.208.167:26657";
const NODE: &str = "http://34.145.158.212:26657";
const CHAIN_ID: &str = "onomy-testnet-1";
const MNEMONIC: &str = include_str!("./../../../../testnet_dealer_mnemonic.txt");

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

async fn onomyd_runner(args: &Args) -> Result<()> {
    // http://34.145.158.212:26657/validators?
    let daemon_home = args.daemon_home.as_ref().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test", &[])
        .await
        .stack()?;

    sh_cosmovisor("query block", &[]).await.stack()?;
    sh_cosmovisor("query slashing signing-infos", &[])
        .await
        .stack()?;

    let comres = Command::new(
        &format!("{daemon_home}/cosmovisor/current/bin/onomyd keys add validator --recover"),
        &[],
    )
    .run_with_input_to_completion(MNEMONIC.as_bytes())
    .await
    .stack()?;
    comres.assert_success().stack()?;

    sleep(TIMEOUT).await;

    //cosmovisor run tx bank send validator
    // onomy1tmtdfh2wm343nkk4424jqe9n0j0ecw870qd9c2 1000000000000000000000anom -y -b
    // block --from validator

    //100000000000000000000000000
    //     1000000000000000000000anom

    //onomy1yks83spz6lvrrys8kh0untt22399tskk6jafcv

    Ok(())
}
