use common::{container_runner, dockerfile_onexd};
use log::info;
use onomy_test_lib::{
    onomy_std_init,
    super_orchestrator::{
        sh,
        stacked_errors::{Error, Result, StackableErr},
        Command, FileOptions,
    },
    Args,
};
use tokio::io::AsyncReadExt;

const FILE: &str =
    include_str!("./../../../../environments/testnet/onex-testnet-3/partial-genesis.json");

async fn get_hash(bytes: &[u8]) -> Result<()> {
    let comres = Command::new("openssl dgst -binary -sha256", &[])
        .ci_mode(true)
        .run_with_input_to_completion(bytes)
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let comres = Command::new("openssl base64 -A", &[])
        .ci_mode(true)
        .run_with_input_to_completion(&comres.stdout)
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let mut out = comres.stdout_as_utf8().stack()?.trim().to_owned();
    // not sure why this needs to be done
    out.pop().unwrap();
    out.push('=');
    println!("{out}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let logs_dir = "./tests/logs";
    let args = onomy_std_init()?;

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "runner" => runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        let file = FILE;
        info!("GENESIS HASH");
        get_hash(file.as_bytes()).await.stack()?;

        container_runner(&args, &[("runner", &dockerfile_onexd())])
            .await
            .stack()?;

        let mut file = FileOptions::read(&format!("{logs_dir}/onexd"))
            .acquire_file()
            .await
            .stack()?;
        let mut contents = vec![];
        file.read_to_end(&mut contents).await.stack()?;

        info!("BINARY HASH");
        get_hash(&contents).await.stack()?;

        Ok(())
    }
}

async fn runner(args: &Args) -> Result<()> {
    let daemon_home = &args.daemon_home.clone().stack()?;

    sh(
        &format!("cp {daemon_home}/cosmovisor/current/bin/onexd /logs/onexd"),
        &[],
    )
    .await
    .stack()?;

    Ok(())
}
