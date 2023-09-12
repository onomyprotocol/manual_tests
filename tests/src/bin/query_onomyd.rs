use common::{container_runner, dockerfile_onomyd};
use onomy_test_lib::{
    cosmovisor::{cosmovisor_get_addr, sh_cosmovisor},
    ibc::{IbcPair, IbcSide},
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        Command,
    },
    Args, TIMEOUT,
};
use tokio::time::sleep;

// some testnet node
//const NODE: &str = "http://34.134.208.167:26657";
const NODE: &str = "http://34.145.158.212:26657";
const CHAIN_ID: &str = "onomy-testnet-1";
const MNEMONIC: &str = include_str!("./../../../../testnet_dealer_mnemonic.txt");

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

async fn onomyd_runner(args: &Args) -> Result<()> {
    let daemon_home = args.daemon_home.as_ref().stack()?;

    sh_cosmovisor("config node", &[NODE]).await.stack()?;
    sh_cosmovisor("config chain-id", &[CHAIN_ID])
        .await
        .stack()?;
    sh_cosmovisor("config keyring-backend test", &[])
        .await
        .stack()?;

    let comres = Command::new(
        &format!("{daemon_home}/cosmovisor/current/bin/onomyd keys add validator --recover"),
        &[],
    )
    .run_with_input_to_completion(MNEMONIC.as_bytes())
    .await
    .stack()?;
    comres.assert_success().stack()?;

    let _addr = &cosmovisor_get_addr("validator").await.stack()?;

    let _ibc_pair = IbcPair {
        a: IbcSide {
            chain_id: "onex-testnet-1".to_owned(),
            connection: "connection-0".to_owned(),
            transfer_channel: "channel-1".to_owned(),
            ics_channel: "channel-0".to_owned(),
        },
        b: IbcSide {
            chain_id: CHAIN_ID.to_owned(),
            connection: "connection-12".to_owned(),
            transfer_channel: "channel-4".to_owned(),
            ics_channel: "channel-3".to_owned(), // ?
        },
    };

    /*
        Chain: onomy-testnet-1
      - Client: 07-tendermint-4
        * Connection: connection-12
          | State: OPEN
          | Counterparty state: OPEN
          + Channel: channel-3
            | Port: provider
            | State: OPEN
            | Counterparty: channel-0
          + Channel: channel-4
            | Port: transfer
            | State: OPEN
            | Counterparty: channel-1
    # Chain: onex-testnet-1
      - Client: 07-tendermint-0
        * Connection: connection-0
          | State: OPEN
          | Counterparty state: OPEN
          + Channel: channel-0
            | Port: consumer
            | State: OPEN
            | Counterparty: channel-3
          + Channel: channel-1
            | Port: transfer
            | State: OPEN
            | Counterparty: channel-4
         */

    sleep(TIMEOUT).await;

    Ok(())
}
