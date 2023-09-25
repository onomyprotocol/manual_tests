use std::time::Duration;

use common::{
    container_runner,
    contest::{get_txs, Record},
    dockerfile_onexd, get_private_key,
};
use deep_space::{Address, Coin};
use log::info;
use onomy_test_lib::{
    cosmovisor::{cosmovisor_get_addr, sh_cosmovisor},
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        Command,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;
use u64_array_bigints::u256;

const NODE: &str = "http://34.145.158.212:36657";
const CHAIN_ID: &str = "onex-testnet-2";
const MNEMONIC: &str = include_str!("./../../../../testnet_dealer_mnemonic.txt");
const RECORDS: &str = include_str!("./../../resources/onex-testnet-trade-war-filtered.csv");

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    Err(Error::from("do not comment out unless ready for send")).stack()?;

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

    let comres = Command::new(
        &format!("{daemon_home}/cosmovisor/current/bin/onexd keys add validator --recover"),
        &[],
    )
    .run_with_input_to_completion(MNEMONIC.as_bytes())
    .await
    .stack()?;
    comres.assert_success().stack()?;

    let contact = deep_space::Contact::new("http://127.0.0.1:9090", TIMEOUT, "onomy").stack()?;
    dbg!(contact.query_total_supply().await.stack()?);

    let private_key = get_private_key(MNEMONIC).stack()?;
    assert_eq!(
        &private_key
            .to_address("onomy")
            .stack()?
            .to_bech32("onomy")
            .stack()?,
        addr
    );

    let records: Vec<Record> = ron::from_str(RECORDS).stack()?;
    let msgs = get_txs(private_key, &records).stack()?;

    for record in &records {
        let balances = contact
            .get_balances(Address::from_bech32(record.addr.clone()).stack()?)
            .await
            .stack()?;
        if !balances.is_empty() {
            dbg!(record, balances);
            panic!();
        }
    }

    // try submitting in one big batch
    info!("submitting batch");
    contact
        .send_message(
            &msgs,
            None,
            &[Coin {
                denom: "anom".to_string(),
                amount: u256!(1_000_000_000),
            }],
            Some(Duration::from_secs(20)),
            private_key,
        )
        .await
        .stack()?;

    info!("double checking");

    for record in &records {
        let balances = contact
            .get_balances(Address::from_bech32(record.addr.clone()).stack()?)
            .await
            .stack()?;
        assert!(balances.len() >= 5);
    }

    info!("successful");

    sleep(Duration::ZERO).await;

    Ok(())
}
