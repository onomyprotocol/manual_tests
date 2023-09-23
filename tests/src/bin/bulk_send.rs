use common::{
    container_runner,
    contest::{precheck_all_batches, Record},
    dockerfile_onexd,
};
use log::info;
use onomy_test_lib::{
    cosmovisor::{cosmovisor_get_addr, sh_cosmovisor},
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        FileOptions,
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

    let records_s =
        FileOptions::read_to_string("./tests/resources/onex-testnet-trade-war-filtered.csv")
            .await
            .stack()?;
    let records: Vec<Record> = ron::from_str(&records_s).stack()?;

    precheck_all_batches(&records).stack()?;
    Err(Error::from("lkj")).stack()?;

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
    // have a guard to prevent accidents
    let addr = &cosmovisor_get_addr("validator").await.stack()?;
    info!("ADDR: {addr}");
    assert_eq!(addr, "onomy1ygphmh38dv64ggh4ayvwczk7pf2u240tkl6ntf");

    let daemon_home = args.daemon_home.clone().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test", &[])
        .await
        .stack()?;

    /*let comres = Command::new(
        &format!("{daemon_home}/cosmovisor/current/bin/onexd keys add validator --recover"),
        &[],
    )
    .run_with_input_to_completion(MNEMONIC.as_bytes())
    .await
    .stack()?;
    comres.assert_success().stack()?;*/

    //cosmovisor run tx bank send validator
    // onomy1ll7pqzg9zscytvj9dmkl3kna50k0fundct62s7 1anom -y -b block --from
    // validator
    sleep(TIMEOUT).await;

    Ok(())
}
