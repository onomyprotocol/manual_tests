use std::collections::HashSet;

use common::MODULE_ACCOUNTS;
use onomy_test_lib::{
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Result, StackableErr},
        FileOptions,
    },
};
use serde::ser::Serialize;
use serde_json::{json, ser::PrettyFormatter, Serializer, Value};

/*const PROPOSAL: &str =
include_str!("./../../../../environments/testnet/onex-testnet-b/genesis-proposal.json");*/
const PARTIAL_GENESIS_WITHOUT_ACCOUNTS: &str = include_str!(
    "./../../../../environments/testnet/onex-testnet-2/partial-genesis-without-accounts.json"
);
const ACCOUNTS: &str = include_str!("./../../../../environments/testnet/accounts.json");
const PARTIAL_GENESIS_PATH: &str = "./../environments/testnet/onex-testnet-2/partial-genesis.json";

#[tokio::main]
async fn main() -> Result<()> {
    let _args = onomy_std_init()?;
    //let logs_dir = "./tests/logs";

    // must remove these
    let module_accounts = MODULE_ACCOUNTS;
    let module_accounts: HashSet<&str> = module_accounts.iter().cloned().collect();

    let mut genesis: Value = serde_json::from_str(PARTIAL_GENESIS_WITHOUT_ACCOUNTS).stack()?;
    let accounts: Value = serde_json::from_str(ACCOUNTS).stack()?;
    let accounts: &[Value] = accounts["account_balances"].as_array().unwrap();

    for account in accounts {
        let address = &account["address"];
        if module_accounts.contains(address.as_str().unwrap()) {
            continue
        }
        let balance = &account["balance"];
        genesis["app_state"]["auth"]["accounts"]
            .as_array_mut()
            .unwrap()
            .push(json!(
                {
                    "@type": "/cosmos.auth.v1beta1.BaseAccount",
                    "address": address,
                    "pub_key": null,
                    "account_number": "0",
                    "sequence": "0"
                }
            ));
        genesis["app_state"]["bank"]["balances"]
            .as_array_mut()
            .unwrap()
            .push(json!(
                {
                "address": address,
                "coins": [
                    {
                        "denom": "aonex",
                        "amount": balance
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

    FileOptions::write_str(PARTIAL_GENESIS_PATH, &genesis_s)
        .await
        .stack()?;

    Ok(())
}
