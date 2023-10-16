use std::{cmp::min, time::Duration};

use common::{
    container_runner,
    contest::{get_txs, Record},
    dockerfile_onomyd, get_private_key,
};
use deep_space::{Address, Coin};
use log::info;
use onomy_test_lib::{
    cosmovisor::{cosmovisor_get_addr, cosmovisor_start},
    onomy_std_init,
    setups::{onomyd_setup, CosmosSetupOptions},
    super_orchestrator::{
        sh,
        stacked_errors::{Error, Result, StackableErr},
        FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;
use u64_array_bigints::u256;

const MNEMONIC: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon \
                        abandon abandon about ";
const RECORDS_PATH: &str = "resources/onex-trade-war-filtered.ron";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onomyd" => onomyd_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        sh("make --directory ./../onomy/ build", &[])
            .await
            .stack()?;
        // copy to dockerfile resources (docker cannot use files from outside cwd)
        sh(
            "cp ./../onomy/onomyd ./tests/dockerfiles/dockerfile_resources/onomyd",
            &[],
        )
        .await
        .stack()?;
        container_runner(&args, &[("onomyd", &dockerfile_onomyd())])
            .await
            .stack()
    }
}

async fn onomyd_runner(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().stack()?;
    let mut options = CosmosSetupOptions::new(daemon_home);
    options.mnemonic = Some(MNEMONIC.to_owned());
    options.onex_testnet_amounts = true;
    onomyd_setup(options).await.stack()?;

    let mut cosmovisor_runner = cosmovisor_start("onomyd_runner.log", None).await.stack()?;

    // guard to prevent accidents
    let addr = &cosmovisor_get_addr("validator").await.stack()?;
    info!("ADDR: {addr}");
    assert_eq!(addr, "onomy19rl4cm2hmr8afy4kldpxz3fka4jguq0axpetws");

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

    /*
    let txn_file = "/logs/txn_send.json";
    let txn_signed_file = "/logs/txn_send_signed.json";
    let chain_id = "onomy";
    let txn_send = sh_cosmovisor("tx bank send onomy1ygphmh38dv64ggh4ayvwczk7pf2u240tkl6ntf
    onomy1a69w3hfjqere4crkgyee79x2mxq0w2pfj9tu2m 1337anom --fees 1000000anom --generate-only", &[])
    .await.stack()?;
    FileOptions::write_str(txn_file, &txn_send).await.stack()?;
    let txn_send_signed = sh_cosmovisor("tx sign", &[txn_file, "--chain-id", chain_id, "--from",
    "validator"]).await.stack()?;
    FileOptions::write_str(txn_signed_file, &txn_send_signed).await.stack()?;
    sh_cosmovisor_tx("broadcast -b block", &[txn_signed_file]).await.stack()?;
    info!("done");
    */

    /*
    let send = MsgSend {
        amount: vec![Coin {
            denom: "anom".to_string(),
            amount: u256!(1337),
        }
        .into()],
        from_address: addr.to_owned(),
        to_address: "onomy1a69w3hfjqere4crkgyee79x2mxq0w2pfj9tu2m".to_string(),
    };

    let msg = Msg::new("/cosmos.bank.v1beta1.MsgSend", send);

    contact
        .simulate_tx(&[msg.clone()], private_key)
        .await
        .stack()?;
    contact
        .send_message(
            &[msg],
            None,
            &[Coin {
                denom: "anom".to_string(),
                amount: u256!(1000000),
            }
            .into()],
            Some(Duration::from_secs(10)),
            private_key,
        )
        .await
        .stack()?;
    */

    let records = FileOptions::read_to_string(RECORDS_PATH).await.stack()?;
    let records: Vec<Record> = ron::from_str(&records).stack()?;
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
        assert!(balances.len() >= 5);
    }

    info!("successful");

    sleep(Duration::ZERO).await;

    cosmovisor_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
