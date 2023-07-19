use std::time::Duration;

use common::container_runner;
use log::info;
use onomy_test_lib::{
    cosmovisor::{
        cosmovisor_get_addr, cosmovisor_gov_file_proposal, cosmovisor_start, get_apr_annual,
        get_delegations_to, get_staking_pool, get_treasury, get_treasury_inflation_annual,
        sh_cosmovisor, sh_cosmovisor_no_dbg, sh_cosmovisor_tx, wait_for_num_blocks,
    },
    dockerfiles::onomy_std_cosmos_daemon,
    onomy_std_init, reprefix_bech32,
    setups::market_standaloned_setup,
    super_orchestrator::{
        sh,
        stacked_errors::{MapAddError, Result},
        Command, FileOptions,
    },
    token18, yaml_str_to_json_value, Args, ONOMY_IBC_NOM, TIMEOUT,
};
use tokio::time::sleep;

const CHAIN_ID: &str = "appname";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        let name = &format!("{CHAIN_ID}d");
        if s.as_str() == name {
            standalone_runner(&args).await
        } else {
            format!("entry_name \"{s}\" is not recognized").map_add_err(|| ())
        }
    } else {
        let mut cmd = Command::new("go build ./cmd/standalone", &[]).ci_mode(true);
        cmd.cwd = Some("./../multiverse/".to_owned());
        let comres = cmd.run_to_completion().await?;
        comres.assert_success()?;
        sh("cd ./../manual_test/", &[]).await?;
        // copy to dockerfile resources (docker cannot use files from outside cwd)
        sh(
            &format!(
                "cp ./../multiverse/standalone \
                 ./tests/dockerfiles/dockerfile_resources/{CHAIN_ID}d"
            ),
            &[],
        )
        .await?;
        container_runner(&args, &[(
            &format!("{CHAIN_ID}d"),
            &onomy_std_cosmos_daemon(
                &format!("{CHAIN_ID}d"),
                &format!(".onomy_{CHAIN_ID}"),
                "v0.1.0",
                &format!("{CHAIN_ID}d"),
            ),
        )])
        .await
    }
}

async fn standalone_runner(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().map_add_err(|| ())?;
    market_standaloned_setup(daemon_home).await?;
    let mut cosmovisor_runner = cosmovisor_start(&format!("{CHAIN_ID}d_runner.log"), None).await?;

    let addr = &cosmovisor_get_addr("validator").await?;
    let valoper_addr = &reprefix_bech32(addr, "onomyvaloper").unwrap();
    info!("valoper address: {valoper_addr}");

    sh_cosmovisor_tx(
        &format!(
            "staking delegate {valoper_addr} 1000000000000000000000anom --gas auto \
             --gas-adjustment 1.3 -y -b block --from validator"
        ),
        &[],
    )
    .await?;
    sh_cosmovisor("query staking validators", &[]).await?;

    info!("{}", get_apr_annual(valoper_addr).await?);

    info!("{}", get_delegations_to(valoper_addr).await?);
    info!("{:?}", get_staking_pool().await?);
    info!("{}", get_treasury().await?);
    info!("{}", get_treasury_inflation_annual().await?);
    info!("{}", get_apr_annual(valoper_addr).await?);

    wait_for_num_blocks(1).await?;
    info!("{}", get_apr_annual(valoper_addr).await?);

    sh(
        &format!(
            "cosmovisor run tx bank send {addr} onomy1a69w3hfjqere4crkgyee79x2mxq0w2pfj9tu2m \
             1337anom --gas auto --gas-adjustment 1.3 -y -b block"
        ),
        &[],
    )
    .await?;

    //cosmovisor run tx staking delegate onomyvaloper
    // 10000000000000000000000ibc/
    // 0EEDE4D6082034D6CD465BD65761C305AACC6FCA1246F87D6A3C1F5488D18A7B --gas auto
    // --gas-adjustment 1.3 -y -b block

    let test_crisis_denom = ONOMY_IBC_NOM;
    let test_deposit = token18(2000.0, "anom");
    cosmovisor_gov_file_proposal(
        daemon_home,
        "param-change",
        &format!(
            r#"
    {{
        "title": "Parameter Change",
        "description": "Making a parameter change",
        "changes": [
          {{
            "subspace": "crisis",
            "key": "ConstantFee",
            "value": {{"denom":"{test_crisis_denom}","amount":"1337"}}
          }}
        ],
        "deposit": "{test_deposit}"
    }}
    "#
        ),
        "1anom",
    )
    .await?;
    wait_for_num_blocks(1).await?;
    // just running this for debug, param querying is weird because it is json
    // inside of yaml, so we will instead test the exported genesis
    sh_cosmovisor("query params subspace crisis ConstantFee", &[]).await?;

    sleep(Duration::ZERO).await;
    cosmovisor_runner.terminate(TIMEOUT).await?;
    // test that exporting works
    let exported = sh_cosmovisor_no_dbg("export", &[]).await?;
    FileOptions::write_str(&format!("/logs/{CHAIN_ID}d_export.json"), &exported).await?;
    let exported = yaml_str_to_json_value(&exported)?;
    assert_eq!(
        exported["app_state"]["crisis"]["constant_fee"]["denom"],
        test_crisis_denom
    );
    assert_eq!(
        exported["app_state"]["crisis"]["constant_fee"]["amount"],
        "1337"
    );

    Ok(())
}
