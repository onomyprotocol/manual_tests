use std::time::Duration;

use common::container_runner;
use log::info;
use onomy_test_lib::{
    cosmovisor::{
        cosmovisor_get_addr, cosmovisor_get_balances, cosmovisor_start, sh_cosmovisor,
        sh_cosmovisor_no_dbg, sh_cosmovisor_tx,
    },
    dockerfiles::onomy_std_cosmos_daemon,
    onomy_std_init,
    setups::market_standalone_setup,
    super_orchestrator::{
        sh,
        stacked_errors::{Error, Result, StackableErr},
        Command, FileOptions,
    },
    u64_array_bigints::{self, u256, U256},
    Args, TIMEOUT,
};
use tokio::time::sleep;

const CHAIN_ID: &str = "market";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "standalone" => standalone_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        let mut cmd = Command::new(&format!("go build ./cmd/{CHAIN_ID}d"), &[]).ci_mode(true);
        cmd.cwd = Some("./../market/".to_owned());
        let comres = cmd.run_to_completion().await.stack()?;
        comres.assert_success()?;
        // copy to dockerfile resources (docker cannot use files from outside cwd)
        sh(
            &format!(
                "cp ./../market/{CHAIN_ID}d ./tests/dockerfiles/dockerfile_resources/{CHAIN_ID}d"
            ),
            &[],
        )
        .await
        .stack()?;
        container_runner(&args, &[(
            "standalone",
            &onomy_std_cosmos_daemon(
                &format!("{CHAIN_ID}d"),
                &format!(".{CHAIN_ID}"),
                "v0.1.0",
                &format!("{CHAIN_ID}d"),
            ),
        )])
        .await
        .stack()
    }
}

pub struct CoinPair {
    coin_a: String,
    coin_b: String,
}

impl CoinPair {
    pub fn new(coin_a: &str, coin_b: &str) -> Result<Self> {
        if coin_a >= coin_b {
            Err(Error::from("coin_a >= coin_b, should be coin_a < coin_b"))
        } else {
            Ok(CoinPair {
                coin_a: coin_a.to_owned(),
                coin_b: coin_b.to_owned(),
            })
        }
    }

    pub fn coin_a(&self) -> &str {
        &self.coin_a
    }

    pub fn coin_b(&self) -> &str {
        &self.coin_b
    }

    pub fn coin_a_amount(&self, amount: U256) -> String {
        format!("{}{}", amount, self.coin_a())
    }

    pub fn coin_b_amount(&self, amount: U256) -> String {
        format!("{}{}", amount, self.coin_b())
    }

    pub fn paired_amounts(&self, amount_a: U256, amount_b: U256) -> String {
        format!(
            "{}{},{}{}",
            amount_a,
            self.coin_a(),
            amount_b,
            self.coin_b()
        )
    }

    pub fn paired(&self) -> String {
        format!("{},{}", self.coin_a(), self.coin_b())
    }

    pub async fn cosmovisor_get_balances(&self, addr: &str) -> Result<(U256, U256)> {
        let balances = cosmovisor_get_balances(addr)
            .await
            .stack_err(|| "cosmovisor_get_balances failed")?;
        let balance_a = *balances
            .get(self.coin_a())
            .stack_err(|| "did not find nonzero coin_a balance")?;
        let balance_b = *balances
            .get(self.coin_b())
            .stack_err(|| "did not find nonzero coin_b balance")?;
        Ok((balance_a, balance_b))
    }
}

// cosmovisor run tx market create-pool 10000000anative 10000000afootoken --from
// validator --fees 1000000anative -y -b block

/// Initiates the pool with 1 of each coin
pub async fn market_create_pool(
    coin_pair: &CoinPair,
    coin_a_amount: U256,
    coin_b_amount: U256,
) -> Result<()> {
    sh_cosmovisor_tx("market create-pool", &[
        &coin_pair.coin_a_amount(coin_a_amount),
        &coin_pair.coin_b_amount(coin_b_amount),
        "--from",
        "validator",
        "--fees",
        "1000000anative",
        "-y",
        "-b",
        "block",
    ])
    .await
    .stack()?;
    Ok(())
}

// cosmovisor run tx market create-pool
// 340282366920938463463374607431768211455anative
// 340282366920938463463374607431768211455afootoken --from validator --fees
// 1000000anative -y -b block cosmovisor run tx market create-pool 1000afootoken
// 1000anative --from validator --fees 1000000anative -y -b block

//pool:
//  denom1: afootoken
//  denom2: anative
//  drops: "2"
//  leaders:
//  - address: onomy1nvsmtc4trpwxrx4vyzlm4ex6e4q3y46wwyapr9 drops: "2"
//  pair: afootoken,anative
pub async fn market_show_pool(coin_pair: &CoinPair) -> Result<String> {
    sh_cosmovisor("query market pool", &[&coin_pair.paired()])
        .await
        .stack()
}

// shows both sides, with one looking like
//member:
//  balance: "1"
//  denomA: anative
//  denomB: afootoken
//  limit: "0"
//  pair: afootoken,anative
//  previous: "0"
//  stop: "0"
pub async fn market_show_members(coin_pair: &CoinPair) -> Result<(String, String)> {
    let member_a = sh_cosmovisor("query market show-member", &[
        coin_pair.coin_a(),
        coin_pair.coin_b(),
    ])
    .await
    .stack()?;
    let member_b = sh_cosmovisor("query market show-member", &[
        coin_pair.coin_b(),
        coin_pair.coin_a(),
    ])
    .await
    .stack()?;
    Ok((member_a, member_b))
}

pub async fn market_create_drop(coin_pair: &CoinPair, drops: U256) -> Result<()> {
    sh_cosmovisor_tx("market create-drop", &[
        &coin_pair.paired(),
        &format!("{}", drops),
        "--from",
        "validator",
        "--fees",
        "1000000anative",
        "-y",
        "-b",
        "block",
    ])
    .await
    .stack()?;
    Ok(())
}

// cosmovisor run tx market create-drop anative,afootoken 1231241 --from
// validator --fees 1000000anative -y -b block cosmovisor run tx market
// create-drop afootoken,anative 1231241 --from validator --fees 1000000anative
// -y -b block

/*
cosmovisor run query market list-drop
drop:
- active: true
  drops: "2"
  owner: onomy1r5q7yrqexn7dyy9uvf3p28raw7mxhc23jwura8
  pair: afootoken,anative
  sum: "2"
  uid: "1"
pagination:
  next_key: null
  total: "0"
*/

//cosmovisor run tx market redeem-drop 1 --from validator --fees 1000000anative
// -y -b block
pub async fn market_redeem_drop(uid: u64) -> Result<()> {
    sh_cosmovisor_tx("market redeem-drop", &[
        &format!("{}", uid),
        "--from",
        "validator",
        "--gas",
        "300000",
        "--fees",
        "1000000anative",
        "-y",
        "-b",
        "block",
    ])
    .await
    .stack()?;
    Ok(())
}

pub async fn market_market_order(
    coin_ask: &str,
    coin_bid: &str,
    amount_bid: U256,
    slippage: u16,
) -> Result<()> {
    sh_cosmovisor_tx("market market-order", &[
        coin_ask,
        coin_bid,
        &format!("{}", amount_bid),
        &format!("{}", slippage),
        "--from",
        "validator",
        "--fees",
        "1000000anative",
        "-y",
        "-b",
        "block",
    ])
    .await
    .stack()?;
    Ok(())
}

// cosmovisor run tx market create-order afootoken anative stop 100 1100,900 0 0
// --from validator --fees 1000000anative -y -b block
pub async fn market_create_order(
    coin_ask: &str,
    coin_bid: &str,
    order_type: &str,
    amount: U256,
    rate: (u64, u64),
    prev: u64,
    next: u64,
) -> Result<()> {
    sh_cosmovisor_tx("market create-order", &[
        coin_ask,
        coin_bid,
        order_type,
        &format!("{}", amount),
        &format!("{},{}", rate.0, rate.1),
        &format!("{}", prev),
        &format!("{}", next),
        "--from",
        "validator",
        "--fees",
        "1000000anative",
        "-y",
        "-b",
        "block",
    ])
    .await
    .stack()?;
    Ok(())
}

pub async fn market_cancel_order(uid: u64) -> Result<()> {
    sh_cosmovisor_tx("market cancel-order", &[
        &format!("{}", uid),
        "--from",
        "validator",
        "--fees",
        "1000000anative",
        "-y",
        "-b",
        "block",
    ])
    .await
    .stack()?;
    Ok(())
}

//cosmovisor run tx market market-order afootoken anative 10000 9999 --from
// validator --fees 1000000anative -y -b block

async fn standalone_runner(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().stack()?;
    market_standalone_setup(daemon_home, CHAIN_ID)
        .await
        .stack()?;
    let mut cosmovisor_runner = cosmovisor_start(&format!("{CHAIN_ID}d_runner.log"), None).await?;

    let addr = &cosmovisor_get_addr("validator").await.stack()?;
    info!("{:?}", cosmovisor_get_balances(addr).await.stack()?);
    let coin_pair = CoinPair::new("afootoken", "anative").stack()?;

    // test numerical limits
    let large = u256!(5192296858534827628530496329220095);
    let large_squared = u256!(26959946667150639794667015087019620289043427352885315420110951809025);
    market_create_pool(&coin_pair, large, large).await.stack()?;
    market_create_drop(&coin_pair, large_squared)
        .await
        .stack()?;
    market_show_pool(&coin_pair).await.stack()?;
    market_market_order(&coin_pair.coin_a, &coin_pair.coin_b, large, 5000)
        .await
        .stack()?;
    market_redeem_drop(1).await.stack()?;
    market_create_order(
        coin_pair.coin_a(),
        coin_pair.coin_b(),
        "stop",
        large,
        (1100, 900),
        0,
        0,
    )
    .await
    .stack()?;
    //market_cancel_order(1).await.stack()?;

    // TODO test empty pool
    /*market_show_members(&coin_pair).await.stack()?;

    let b0 = coin_pair.cosmovisor_get_balances(addr).await.stack()?.0;
    //market_create_drop(&coin_pair, 1).await.stack()?;
    market_create_drop(&coin_pair, 12345).await.stack()?;
    let b1 = coin_pair.cosmovisor_get_balances(addr).await.stack()?.0;
    info!("change: {}", b0 - b1);

    market_show_members(&coin_pair).await.stack()?;

    // cosmovisor run tx market redeem-drop 2 --from validator --fees 1000000anative
    // -y -b block
    //sleep(TIMEOUT).await;

    market_redeem_drop(2).await.stack()?;
    market_redeem_drop(1).await.stack()?;

    market_show_members(&coin_pair).await.stack()?;*/

    //cosmovisor run query market bookends
    // cosmovisor run tx market create-order [denom-ask] [denom-bid] [order-type]
    // [amount] [rate] [prev] [next] [flags]

    //sleep(TIMEOUT).await;
    sleep(Duration::ZERO).await;
    cosmovisor_runner.terminate(TIMEOUT).await.stack()?;
    // test that exporting works
    let exported = sh_cosmovisor_no_dbg("export", &[]).await.stack()?;
    FileOptions::write_str(&format!("/logs/{CHAIN_ID}d_export.json"), &exported)
        .await
        .stack()?;

    Ok(())
}
