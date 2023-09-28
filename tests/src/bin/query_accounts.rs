use common::{container_runner, dockerfile_onexd};
use onomy_test_lib::{
    cosmovisor::{sh_cosmovisor, sh_cosmovisor_no_dbg},
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        FileOptions,
    },
    yaml_str_to_json_value, Args,
};

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

    let accounts = sh_cosmovisor_no_dbg("query auth accounts --limit 100", &[])
        .await
        .stack()?;

    FileOptions::write_str(
        "/resources/query_accounts.json",
        &yaml_str_to_json_value(&accounts).stack()?.to_string(),
    )
    .await
    .stack()?;

    Ok(())
}
