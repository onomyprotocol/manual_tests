use onomy_test_lib::super_orchestrator::{
    stacked_errors::{Result, StackableErr},
    std_init, Command, FileOptions,
};

#[tokio::main]
async fn main() -> Result<()> {
    std_init()?;

    let genesis = FileOptions::read_to_string("./tests/resources/onex-testnet-genesis.json")
        .await
        .stack()?;
    let comres = Command::new("openssl dgst -binary -sha256", &[])
        .ci_mode(true)
        .run_with_input_to_completion(genesis.as_bytes())
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let comres = Command::new("openssl base64 -A", &[])
        .ci_mode(true)
        .run_with_input_to_completion(&comres.stdout)
        .await
        .stack()?;
    comres.assert_success().stack()?;
    println!("{}", comres.stdout_as_str().stack()?);

    Ok(())
}
