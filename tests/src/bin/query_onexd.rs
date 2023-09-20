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
const CHAIN_ID: &str = "onex-testnet-2";
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

    // in order to access the 1317 port locally, use `docker inspect` to find the IP
    // address of the container from the host
    // http://172.21.0.2:1317/
    // may need to use
    //enable_swagger_apis(daemon_home).await.stack()?;
    // but note it may take over a minute to start up

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

    //cosmovisor run tx bank send validator
    // onomy1ll7pqzg9zscytvj9dmkl3kna50k0fundct62s7 1anom -y -b block --from
    // validator

    sleep(TIMEOUT).await;

    Ok(())
}
