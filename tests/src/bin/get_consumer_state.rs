use common::{container_runner, dockerfile_onomyd};
use onomy_test_lib::{
    cosmovisor::sh_cosmovisor,
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        FileOptions,
    },
    yaml_str_to_json_value, Args,
};
use serde::ser::Serialize;
use serde_json::{ser::PrettyFormatter, Serializer, Value};

const ONOMY_NODE: &str = "http://34.28.227.180:26657";
const ONOMY_CHAIN_ID: &str = "onomy-testnet-1";
const CONSUMER_CHAIN_ID: &str = "onex-testnet-1";
const PROPOSAL: &str =
    include_str!("./../../../../market/tools/config/testnet/onex-testnet-genesis-proposal.json");
/*const PARTIAL_GENESIS: &str =
include_str!("./../../../../market/tools/config/testnet/onex-testnet-partial-genesis.json");*/

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onomyd" => onomyd_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args, &[("onomyd", &dockerfile_onomyd())])
            .await
            .stack()
    }
}

async fn onomyd_runner(_args: &Args) -> Result<()> {
    //let daemon_home = args.daemon_home.as_ref().stack()?;

    sh_cosmovisor("config node", &[ONOMY_NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[ONOMY_CHAIN_ID])
        .await
        .stack()?;

    let ccvconsumer_state = sh_cosmovisor("query provider consumer-genesis", &[CONSUMER_CHAIN_ID])
        .await
        .stack()?;
    let mut state = yaml_str_to_json_value(&ccvconsumer_state).stack()?;

    let proposal: Value = serde_json::from_str(PROPOSAL).stack()?;

    // fix missing fields TODO when we update canonical versions we should be able
    // to remove this
    state["params"]["soft_opt_out_threshold"] = "0.0".into();
    state["params"]["provider_reward_denoms"] = proposal["provider_reward_denoms"].clone();
    state["params"]["reward_denoms"] = proposal["reward_denoms"].clone();

    //genesis["app_state"]["ccvconsumer"] = state;

    let mut state_s = vec![];
    let formatter = PrettyFormatter::with_indent(&[b' ', b' ']);
    let mut ser = Serializer::with_formatter(&mut state_s, formatter);
    state.serialize(&mut ser).stack()?;
    let state_s = String::from_utf8(state_s).stack()?;

    FileOptions::write_str(
        &format!("/logs/{CONSUMER_CHAIN_ID}_ccvconsumer_state.json"),
        &state_s,
    )
    .await
    .stack()?;

    //sleep(TIMEOUT).await;

    Ok(())
}
