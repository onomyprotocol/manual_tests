use std::{cmp::min, time::Duration};

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
        stacked_errors::{ensure, ensure_eq, Error, Result, StackableErr},
        Command, FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;
use u64_array_bigints::u256;

const NODE: &str = "http://34.86.135.162:26657";
const NODE_GRPC: &str = "http://34.86.135.162:9090";
const CHAIN_ID: &str = "onex-testnet-3";
const MNEMONIC: &str = include_str!("./../../../../testnet_dealer_mnemonic.txt");
const RECORDS_PATH: &str = "/resources/onex-trade-war-filtered.ron";

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
    let daemon_home = args.daemon_home.clone().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test", &[])
        .await
        .stack()?;

    let comres = Command::new(format!(
        "{daemon_home}/cosmovisor/current/bin/onexd keys add validator --recover"
    ))
    .run_with_input_to_completion(MNEMONIC.as_bytes())
    .await
    .stack()?;
    comres.assert_success().stack()?;

    // have a guard to prevent accidents
    let addr = &cosmovisor_get_addr("validator").await.stack()?;
    info!("ADDR: {addr}");
    ensure_eq!(addr, "onomy1yks83spz6lvrrys8kh0untt22399tskk6jafcv");

    let contact = deep_space::Contact::new(NODE_GRPC, TIMEOUT, "onomy").stack()?;
    dbg!(contact.query_total_supply().await.stack()?);

    let private_key = get_private_key(MNEMONIC).stack()?;
    ensure_eq!(
        &private_key
            .to_address("onomy")
            .stack()?
            .to_bech32("onomy")
            .stack()?,
        addr
    );

    let records = FileOptions::read_to_string(RECORDS_PATH).await.stack()?;
    let records: Vec<Record> = ron::from_str(&records).stack()?;
    let msgs = get_txs(private_key, &records).stack()?;

    ensure_eq!(msgs.len(), 3885);

    for (i, record) in records.iter().enumerate() {
        if (i % 100) == 0 {
            info!("checked addr {i}")
        }
        let balances = contact
            .get_balances(Address::from_bech32(record.addr.clone()).stack()?)
            .await
            .stack()?;
        if balances.len() > 1 {
            // make sure they all have only aonex
            dbg!(record, balances);
            panic!();
        }
    }

    const BATCH_SIZE: usize = 1000;
    let batch_start = 0;
    for batch_i in batch_start.. {
        let i_start = batch_i * BATCH_SIZE;
        if i_start >= msgs.len() {
            break
        }
        let i_end = min(i_start + BATCH_SIZE, msgs.len());
        // try submitting in one big batch
        info!("submitting batch {batch_i}");
        contact
            .send_message(
                &msgs[i_start..i_end],
                None,
                &[Coin {
                    denom: "anom".to_string(),
                    amount: u256!(1_000_000_000),
                }],
                Some(Duration::from_secs(20)), // `None` may help with errors
                private_key,
            )
            .await
            .stack()?;
    }
    info!("double checking");

    for record in &records {
        let balances = contact
            .get_balances(Address::from_bech32(record.addr.clone()).stack()?)
            .await
            .stack()?;
        ensure!(balances.len() >= 5);
    }

    info!("successful");

    sleep(Duration::ZERO).await;

    Ok(())
}
