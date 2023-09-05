use onomy_test_lib::super_orchestrator::{Command, FileOptions, stacked_errors::{Result, StackableErr}, std_init};

#[tokio::main]
async fn main() -> Result<()> {
    std_init()?;

    let genesis = FileOptions::read_to_string("./tests/resources/onex-testnet-genesis.json").await.stack()?;
    let comres = Command::new("openssl dgst -binary -sha256", &[]).run_with_input_to_completion(genesis.as_bytes()).await.stack()?;
    comres.assert_success()?;

    // cat ./tests/resources/onex-testnet-genesis.json | openssl dgst -binary -sha256 | openssl base64 -A
    // cat onexd | openssl dgst -binary -sha256 | openssl base64 -A
    // wget https://github.com/onomyprotocol/multiverse/releases/download/v0.1.0.1-onex/onexd

    Ok(())
}
