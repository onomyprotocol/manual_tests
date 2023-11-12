//! not sure what I used this for

use std::collections::{btree_map::Entry, BTreeMap, HashSet};

use common::MODULE_ACCOUNTS;
use onomy_test_lib::{
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Result, StackableErr},
        stacked_get, stacked_get_mut, FileOptions,
    },
};
use serde::ser::Serialize;
use serde_json::{json, ser::PrettyFormatter, Serializer, Value};
use u64_array_bigints::U256;

/*const PROPOSAL: &str =
include_str!("./../../../../environments/testnet/onex-testnet-3/genesis-proposal.json");*/
const PARTIAL_GENESIS_WITHOUT_ACCOUNTS_PATH: &str =
    "./../environments/testnet/onex-testnet-3/partial-genesis-without-accounts.json";
const EXPORTED_GENESIS_PATH: &str = "./../../../Downloads/genesis-exported-testnet-master.json";
const PARTIAL_GENESIS_PATH: &str = "./../environments/testnet/onex-testnet-3/partial-genesis.json";

#[tokio::main]
async fn main() -> Result<()> {
    let _args = onomy_std_init()?;
    //let logs_dir = "./tests/logs";

    // must remove these from accounts
    let module_accounts = MODULE_ACCOUNTS;
    let module_accounts: HashSet<&str> = module_accounts.iter().cloned().collect();

    let partial_genesis_without_accounts =
        FileOptions::read_to_string(PARTIAL_GENESIS_WITHOUT_ACCOUNTS_PATH)
            .await
            .stack()?;
    let exported_genesis = FileOptions::read_to_string(EXPORTED_GENESIS_PATH)
        .await
        .stack()?;

    let exported: Value = serde_json::from_str(&exported_genesis).stack()?;
    let mut genesis: Value = serde_json::from_str(&partial_genesis_without_accounts).stack()?;

    let validators_value: &[Value] = stacked_get!(exported["app_state"]["staking"]["validators"])
        .as_array()
        .stack()?;

    struct Total {
        shares: U256,
        tokens: U256,
    }
    let mut validators: BTreeMap<String, Total> = BTreeMap::new();
    for validator in validators_value {
        let shares = &validator["delegator_shares"];
        let shares = shares.as_str().unwrap();
        // the shares can be fractional, truncate at the decimal point
        let i = shares.find('.').unwrap();
        let shares = &shares[..i];
        let shares = U256::from_dec_or_hex_str(shares).unwrap();
        let tokens = U256::from_dec_or_hex_str(validator["tokens"].as_str().unwrap()).unwrap();
        validators.insert(
            validator["operator_address"].as_str().unwrap().to_owned(),
            Total { shares, tokens },
        );
    }

    // use only bonded amounts
    let delegations: &[Value] = stacked_get!(exported["app_state"]["staking"]["delegations"])
        .as_array()
        .stack()?;

    let mut allocations = BTreeMap::<String, u128>::new();
    for delegation in delegations {
        let address = stacked_get!(delegation["delegator_address"]);
        let address = address.as_str().unwrap();
        if module_accounts.contains(address) {
            // there shouldn't be any modules delegating to anyone
            panic!();
            //continue
        }
        let shares = stacked_get!(delegation["shares"]);
        let shares = shares.as_str().unwrap();
        // the shares can be fractional, truncate at the decimal point
        let i = shares.find('.').unwrap();
        let shares = &shares[..i];
        let shares = U256::from_dec_or_hex_str(shares).unwrap();

        let total = validators
            .get(
                stacked_get!(delegation["validator_address"])
                    .as_str()
                    .stack()?,
            )
            .unwrap();

        // delegated tokens = (shares * total_tokens) / total_shares
        let tmp = shares.checked_mul(total.tokens).unwrap();
        let tmp = tmp.divide(total.shares).unwrap().0;
        let tmp = tmp.try_resize_to_u128().unwrap();

        match allocations.entry(address.to_owned()) {
            Entry::Vacant(v) => {
                v.insert(tmp);
            }
            Entry::Occupied(mut o) => {
                // if multiple delegations from same address, add them up
                *o.get_mut() = o.get().checked_add(tmp).unwrap();
            }
        }
    }

    for (address, allocation) in allocations {
        let allocation = allocation.to_string();
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
                        "denom": "aonex",
                        "amount": allocation
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
