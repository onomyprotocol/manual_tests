use onomy_test_lib::super_orchestrator::{
    stacked_errors::{Result, StackableErr},
    std_init, Command,
};

const FILE: &str =
    include_str!("./../../../../market/tools/config/testnet/onex-testnet-genesis.json");

#[tokio::main]
async fn main() -> Result<()> {
    std_init()?;

    let file = FILE;
    let comres = Command::new("openssl dgst -binary -sha256", &[])
        .ci_mode(true)
        .run_with_input_to_completion(file.as_bytes())
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let comres = Command::new("openssl base64 -A", &[])
        .ci_mode(true)
        .run_with_input_to_completion(&comres.stdout)
        .await
        .stack()?;
    comres.assert_success().stack()?;
    println!("{}", comres.stdout_as_utf8().stack()?);

    //cat ./tests/resources/onex-testnet-genesis.json | openssl dgst -binary
    // -sha256 | openssl base64 -A

    Ok(())
}
