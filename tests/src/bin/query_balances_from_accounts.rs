use std::collections::HashSet;

use common::{container_runner, dockerfile_onexd, MODULE_ACCOUNTS};
use log::info;
use onomy_test_lib::{
    cosmovisor::{sh_cosmovisor, sh_cosmovisor_no_dbg},
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        stacked_get, stacked_get_mut, FileOptions,
    },
    yaml_str_to_json_value, Args,
};
use serde_json::{json, Value};

const NODE: &str = "http://35.239.163.97:26657";
const CHAIN_ID: &str = "onex-devnet-1";

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

async fn onexd_runner(_args: &Args) -> Result<()> {
    //let daemon_home = args.daemon_home.as_ref().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;

    let accounts_s = FileOptions::read_to_string("/resources/query_accounts.json")
        .await
        .stack()?;
    let accounts: Value = serde_json::from_str(&accounts_s).stack()?;

    // must remove these
    let module_accounts = MODULE_ACCOUNTS;
    let module_accounts: HashSet<&str> = module_accounts.iter().cloned().collect();

    let mut accounts_and_balances: Value = Value::Array(vec![]);

    let accounts: &[Value] = stacked_get!(accounts["accounts"]).as_array().stack()?;
    let mut i = 0;
    for account in accounts {
        if i % 100 == 0 {
            info!("reached account {i}");
        }
        let address = stacked_get!(account["address"]);
        if let Some(address) = address.as_str() {
            if module_accounts.contains(address) {
                continue
            }
            let balances = sh_cosmovisor_no_dbg("query bank balances", &[address])
                .await
                .stack()?;
            let mut balances = yaml_str_to_json_value(&balances).stack()?;
            let balances = stacked_get_mut!(balances["balances"]).take();

            accounts_and_balances.as_array_mut().stack()?.push(json!(
                {
                    "address": address,
                    "coins": balances,
                }
            ));
        }
        i += 1;
    }

    FileOptions::write_str(
        "/resources/accounts_and_balances.json",
        &accounts_and_balances.to_string(),
    )
    .await
    .stack()?;

    Ok(())
}
