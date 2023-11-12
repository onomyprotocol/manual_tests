//! acquires consumer state from the provider for use in completing the consumer
//! genesis, automatically overwriting the complete genesis (please commit in
//! the --genesis-path directory before running) with the partial
//! genesis contents with consumer state inserted

use onomy_test_lib::dockerfiles::dockerfile_onomyd;
#[rustfmt::skip]
/*
e.x.

cargo r --bin get_consumer_state -- --proposal-path ./../environments/testnet/onex-testnet-3/genesis-proposal.json --partial-genesis-path ./../environments/testnet/onex-testnet-3/partial-genesis.json --genesis-path ./../environments/testnet/onex-testnet-3/genesis.json --node http://34.28.227.180:26657

*/

use onomy_test_lib::{
    cosmovisor::sh_cosmovisor,
    onomy_std_init,
    super_orchestrator::{
        acquire_file_path,
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{ensure_eq, Error, Result, StackableErr},
        stacked_get, stacked_get_mut, FileOptions,
    },
    yaml_str_to_json_value, Args, TIMEOUT,
};
use serde::ser::Serialize;
use serde_json::{ser::PrettyFormatter, Serializer, Value};

const ONOMY_CHAIN_ID: &str = "onomy-testnet-1";
const CONSUMER_CHAIN_ID: &str = "onex-testnet-3";

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "onomyd" => onomyd_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        let partial_genesis = FileOptions::read_to_string(
            args.partial_genesis_path
                .as_deref()
                .stack_err(|| "need --partial-genesis-path")?,
        )
        .await
        .stack()?;
        let mut genesis: Value = serde_json::from_str(&partial_genesis).stack()?;

        let complete_genesis_path = acquire_file_path(
            args.genesis_path
                .as_deref()
                .stack_err(|| "need --genesis-path")?,
        )
        .await
        .stack()?;

        FileOptions::copy(
            args.proposal_path
                .as_deref()
                .stack_err(|| "need --proposal-path")?,
            "./tests/resources/tmp/proposal.json",
        )
        .await
        .stack()?;

        // read from node
        let logs_dir = "./tests/logs";
        let resources_dir = "./tests/resources";
        let dockerfiles_dir = "./tests/dockerfiles";
        let bin_entrypoint = &args.bin_name;
        let container_target = "x86_64-unknown-linux-gnu";

        // build internal runner
        sh([
            "cargo build --release --bin",
            bin_entrypoint,
            "--target",
            container_target,
        ])
        .await
        .stack()?;

        let mut containers = vec![];
        containers.push(
            Container::new("onomyd", Dockerfile::contents(dockerfile_onomyd()))
                .external_entrypoint(
                    format!("./target/{container_target}/release/{bin_entrypoint}"),
                    [
                        "--entry-name",
                        "onomyd",
                        "--node",
                        args.node.as_deref().stack_err(|| "need --node")?,
                    ],
                )
                .await
                .stack()?,
        );

        let mut cn =
            ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir)
                .stack()?;
        cn.add_common_volumes([(logs_dir, "/logs"), (resources_dir, "/resources")]);
        let uuid = cn.uuid_as_string();
        cn.add_common_entrypoint_args(["--uuid", &uuid]);
        cn.run_all(true).await.stack()?;
        cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
        cn.terminate_all().await;

        // afterwards get the output and write the complete genesis

        let state_s = FileOptions::read_to_string(&format!(
            "./tests/logs/{CONSUMER_CHAIN_ID}_ccvconsumer_state.json"
        ))
        .await
        .stack()?;
        let state: Value = serde_json::from_str(&state_s).stack()?;
        *stacked_get_mut!(genesis["app_state"]["ccvconsumer"]) = state.clone();

        let mut genesis_s = vec![];
        let formatter = PrettyFormatter::with_indent(&[b' ', b' ']);
        let mut ser = Serializer::with_formatter(&mut genesis_s, formatter);
        genesis.serialize(&mut ser).stack()?;
        let genesis_s = String::from_utf8(genesis_s).stack()?;
        FileOptions::write_str(complete_genesis_path, &genesis_s)
            .await
            .stack()?;

        Ok(())
    }
}

async fn onomyd_runner(args: &Args) -> Result<()> {
    //let daemon_home = args.daemon_home.as_ref().stack()?;

    let proposal = FileOptions::read_to_string("/resources/tmp/proposal.json")
        .await
        .stack()?;
    let proposal: Value = serde_json::from_str(&proposal).stack()?;
    ensure_eq!(stacked_get!(proposal["chain_id"]), CONSUMER_CHAIN_ID);

    sh_cosmovisor(["config node", args.node.as_deref().stack()?])
        .await
        .stack()?;
    sh_cosmovisor(["config chain-id", ONOMY_CHAIN_ID])
        .await
        .stack()?;

    let ccvconsumer_state = sh_cosmovisor(["query provider consumer-genesis", CONSUMER_CHAIN_ID])
        .await
        .stack()?;
    let mut state = yaml_str_to_json_value(&ccvconsumer_state).stack()?;

    // fix missing fields TODO when we update canonical versions we should be able
    // to remove this
    stacked_get_mut!(state["params"])["soft_opt_out_threshold"] = "0.0".into();
    stacked_get_mut!(state["params"])["provider_reward_denoms"] =
        stacked_get!(proposal["provider_reward_denoms"]).clone();
    stacked_get_mut!(state["params"])["reward_denoms"] =
        stacked_get!(proposal["reward_denoms"]).clone();

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

    Ok(())
}
