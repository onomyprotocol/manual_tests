use onomy_test_lib::super_orchestrator::{
    stacked_errors::{Result, StackableErr},
    FileOptions,
};
use serde::Serialize;
use serde_json::{json, ser::PrettyFormatter, Serializer, Value};

#[tokio::main]
async fn main() -> Result<()> {
    let genesis_s = FileOptions::read_to_string("./tests/resources/partial_genesis.json")
        .await
        .stack()?;
    let mut genesis: Value = serde_json::from_str(&genesis_s).stack()?;

    let accounts_and_balances_s =
        FileOptions::read_to_string("./tests/resources/accounts_and_balances.json")
            .await
            .stack()?;
    let accounts_and_balances: Value = serde_json::from_str(&accounts_and_balances_s).stack()?;

    let accounts_and_balances = accounts_and_balances.as_array().unwrap();

    for account_and_balance in accounts_and_balances {
        let address = &account_and_balance["address"];
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
            .push(account_and_balance.clone());
    }

    let mut genesis_s = vec![];
    let formatter = PrettyFormatter::with_indent(&[b' ', b' ']);
    let mut ser = Serializer::with_formatter(&mut genesis_s, formatter);
    genesis.serialize(&mut ser).stack()?;
    let genesis_s = String::from_utf8(genesis_s).stack()?;

    FileOptions::write_str("./tests/resources/onex_restart_1_genesis.json", &genesis_s)
        .await
        .stack()?;

    Ok(())
}
