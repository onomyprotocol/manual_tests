//! Given a processed RON file from `process_contest_whitelist.rs`, this can
//! create a contest genesis file

use std::collections::BTreeSet;

use clap::Parser;
use common::contest::Record;
use onomy_test_lib::super_orchestrator::{
    stacked_errors::{Result, StackableErr},
    stacked_get_mut, std_init, FileOptions,
};
use serde::ser::Serialize;
use serde_json::{json, ser::PrettyFormatter, Serializer, Value};

#[derive(Parser, Debug, Clone)]
#[command(about)]
struct Args {
    #[arg(long, default_value_t = String::from("./tests/resources/genesis_input.json"))]
    pub genesis_input: String,
    #[arg(long, default_value_t = String::from("./tests/resources/contest_whitelist.ron"))]
    pub ron_input: String,
    #[arg(long, default_value_t = String::from("./tests/resources/genesis_output.json"))]
    pub genesis_output: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    std_init()?;
    let args = Args::parse();

    let genesis_input = FileOptions::read_to_string(&args.genesis_input)
        .await
        .stack()?;
    let mut genesis: Value = serde_json::from_str(&genesis_input).stack()?;
    let ron_input = FileOptions::read_to_string(&args.ron_input).await.stack()?;
    let list: Vec<Record> = ron::from_str(&ron_input).stack()?;

    let mut base_accounts = BTreeSet::<String>::new();

    for record in &list {
        base_accounts.insert(record.addr.clone());
    }

    for address in base_accounts {
        stacked_get_mut!(genesis["app_state"]["auth"]["accounts"])
            .as_array_mut()
            .stack()?
            .push(json!(
                {
                    "@type": "/cosmos.auth.v1beta1.BaseAccount",
                    "address": address,
                    "pub_key": null,
                    "account_number": "0",
                    "sequence": "0"
                }
            ));
        stacked_get_mut!(genesis["app_state"]["bank"]["balances"])
            .as_array_mut()
            .stack()?
            .push(json!(
                {
                "address": address,
                "coins": [
                    {
                      "denom": "abtc",
                      "amount": "2000000000000000000"
                    },
                    {
                      "denom": "anom",
                      "amount": "1000000000000000000000"
                    },
                    {
                      "denom": "aonex",
                      "amount": "1000000000000000000000"
                    },
                    {
                      "denom": "ausdc",
                      "amount": "10000000000000000000000"
                    },
                    {
                      "denom": "ausdt",
                      "amount": "10000000000000000000000"
                    },
                    {
                      "denom": "wei",
                      "amount": "150000000000000000000"
                    }
                ]
                }
            ));
    }

    let mut genesis_s = vec![];
    let formatter = PrettyFormatter::with_indent(&[b' ', b' ']);
    let mut ser = Serializer::with_formatter(&mut genesis_s, formatter);
    genesis.serialize(&mut ser).stack()?;
    let genesis_s = String::from_utf8(genesis_s).stack()?;

    FileOptions::write_str(&args.genesis_output, &genesis_s)
        .await
        .stack()?;

    Ok(())
}
