//! Run this manually once a week to insure that ICS channels do not expire
//!
//! Check the outputs in the cosole and in ./tests/logs/hermes_ics_runner.log to
//! make sure there are no errors or significant warnings

/*
e.x.

cargo r --bin hermes_manual -- --mnemonic-path ./../testnet_dealer_mnemonic.txt

cargo r --bin auto_exec_i -- --container-name hermes

*/

use onomy_test_lib::{
    dockerfiles::dockerfile_hermes,
    hermes::{hermes_start, sh_hermes},
    onomy_std_init,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Error, Result, StackableErr},
        FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

const HERMES_CONFIG: &str = r##"
# The global section has parameters that apply globally to the relayer operation.
[global]
log_level = 'info'

# Specify the mode to be used by the relayer. [Required]
[mode]

# Specify the client mode.
[mode.clients]

# Whether or not to enable the client workers. [Required]
enabled = true

# Whether or not to enable periodic refresh of clients. [Default: true]
# This feature only applies to clients that underlie an open channel.
# For Tendermint clients, the frequency at which Hermes refreshes them is 2/3 of their
# trusting period (e.g., refresh every ~9 days if the trusting period is 14 days).
# Note: Even if this is disabled, clients will be refreshed automatically if
#      there is activity on a connection or channel they are involved with.
refresh = true

# Whether or not to enable misbehaviour detection for clients. [Default: true]
misbehaviour = false

# Specify the connections mode.
[mode.connections]

# Whether or not to enable the connection workers for handshake completion. [Required]
enabled = false

# Specify the channels mode.
[mode.channels]

# Whether or not to enable the channel workers for handshake completion. [Required]
enabled = false

# Specify the packets mode.
[mode.packets]

# Whether or not to enable the packet workers. [Required]
enabled = true

# Parametrize the periodic packet clearing feature.
# Interval (in number of blocks) at which pending packets
# should be periodically cleared. A value of '0' will disable
# periodic packet clearing. [Default: 100]
clear_interval = 0

# Whether or not to clear packets on start. [Default: true]
clear_on_start = true

# Toggle the transaction confirmation mechanism.
# The tx confirmation mechanism periodically queries the `/tx_search` RPC
# endpoint to check that previously-submitted transactions
# (to any chain in this config file) have been successfully delivered.
# If they have not been, and `clear_interval = 0`, then those packets are
# queued up for re-submission.
# If set to `false`, the following telemetry metrics will be disabled:
# `acknowledgment_packets_confirmed`, `receive_packets_confirmed` and `timeout_packets_confirmed`.
# [Default: false]
tx_confirmation = false

# Auto register the counterparty payee on a destination chain to
# the relayer's address on the source chain. This can be used
# for simple configuration of the relayer to receive fees for
# relaying RecvPacket on fee-enabled channels.
# For more complex configuration, turn this off and use the CLI
# to manually register the payee addresses.
# [Default: false]
auto_register_counterparty_payee = false

# The REST section defines parameters for Hermes' built-in RESTful API.
# https://hermes.informal.systems/rest.html
[rest]

# Whether or not to enable the REST service. Default: false
enabled = true

# Specify the IPv4/6 host over which the built-in HTTP server will serve the RESTful
# API requests. Default: 127.0.0.1
host = '127.0.0.1'

# Specify the port over which the built-in HTTP server will serve the restful API
# requests. Default: 3000
port = 3000


# The telemetry section defines parameters for Hermes' built-in telemetry capabilities.
# https://hermes.informal.systems/telemetry.html
[telemetry]

# Whether or not to enable the telemetry service. Default: false
enabled = false

# Specify the IPv4/6 host over which the built-in HTTP server will serve the metrics
# gathered by the telemetry service. Default: 127.0.0.1
host = '127.0.0.1'

# Specify the port over which the built-in HTTP server will serve the metrics gathered
# by the telemetry service. Default: 3001
port = 3001

[[chains]]
id = 'onomy-mainnet-1'
rpc_addr = 'http://35.224.118.71:26657'
grpc_addr = 'http://35.224.118.71:9191'
event_source = { mode = 'push', url = 'ws://35.224.118.71:26657/websocket', batch_delay = '200ms' }
rpc_timeout = '15s'
account_prefix = 'onomy'
key_name = 'onomy-mainnet-1'
store_prefix = 'ibc'
gas_price = { price = 0, denom = 'anom' }
max_gas = 10000000
clock_drift = '5s'
trusting_period = '7days'
trust_threshold = { numerator = '1', denominator = '3' }

[[chains]]
id = 'osmosis-1'
rpc_addr = 'https://osmosis-rpc.w3coins.io'
grpc_addr = 'http://osmosis-grpc.w3coins.io:12590'
event_source = { mode = 'push', url = 'ws://osmosis-rpc.w3coins.io/websocket', batch_delay = '200ms' }
rpc_timeout = '15s'
account_prefix = 'osmo'
key_name = 'osmosis-1'
store_prefix = 'ibc'
gas_price = { price = 0.025, denom = 'uosmo' }
max_gas = 10000000
clock_drift = '5s'
trusting_period = '7days'
trust_threshold = { numerator = '1', denominator = '3' }

[chains.packet_filter]
policy = 'allow'
list = [[
    'transfer',
    'channel-525',
]]
"##;

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "hermes" => hermes_runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args).await.stack()
    }
}

async fn container_runner(args: &Args) -> Result<()> {
    let logs_dir = "./tests/logs";
    let dockerfiles_dir = "./tests/dockerfiles";
    let bin_entrypoint = &args.bin_name;
    let container_target = "x86_64-unknown-linux-gnu";

    // build internal runner with `--release`
    sh([
        "cargo build --release --bin",
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await
    .stack()?;

    FileOptions::copy(
        args.mnemonic_path
            .as_deref()
            .stack_err(|| "need --mnemonic-path")?,
        "./tests/resources/tmp/mnemonic.txt",
    )
    .await
    .stack()?;

    let entrypoint = &format!("./target/{container_target}/release/{bin_entrypoint}");

    let mut cn = ContainerNetwork::new(
        "test",
        vec![Container::new(
            "hermes",
            Dockerfile::contents(dockerfile_hermes("__tmp_hermes_config.toml")),
        )
        .external_entrypoint(entrypoint, ["--entry-name", "hermes"])
        .await
        .stack()?],
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )
    .stack()?;
    cn.add_common_volumes([(logs_dir, "/logs"), ("./tests/resources/", "/resources/")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(["--uuid", &uuid]);

    FileOptions::write_str(
        &format!("{dockerfiles_dir}/dockerfile_resources/__tmp_hermes_config.toml"),
        HERMES_CONFIG,
    )
    .await?;

    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

async fn hermes_runner(_args: &Args) -> Result<()> {
    let mnemonic = FileOptions::read_to_string("/resources/tmp/mnemonic.txt")
        .await
        .stack()?;

    // set keys for our chains
    FileOptions::write_str("/root/.hermes/dealer_mnemonic.txt", &mnemonic)
        .await
        .stack()?;

    // add the chains
    for id in ["onomy-mainnet-1", "osmosis-1"] {
        sh_hermes([format!(
            "keys add --chain {} --mnemonic-file /root/.hermes/dealer_mnemonic.txt",
            id
        )])
        .await
        .stack()?;
    }

    let mut hermes_runner = hermes_start("/logs/hermes_ics_runner.log").await.stack()?;
    sleep(TIMEOUT).await;
    hermes_runner.terminate(TIMEOUT).await.stack()?;

    Ok(())
}
