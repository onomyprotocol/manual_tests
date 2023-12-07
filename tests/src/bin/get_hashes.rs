//! get the hash of a genesis file and a binary in the logs directory (please
//! refactor if necessary)

/*
e.x.

cargo r --bin get_hashes -- --genesis-path ./../environments/testnet/onex-testnet-4/partial-genesis.json

*/

use common::container_runner;
use log::info;
use onomy_test_lib::{
    dockerfiles::dockerfile_onexd,
    onomy_std_init,
    super_orchestrator::{
        sh,
        stacked_errors::{Error, Result, StackableErr},
        Command, FileOptions,
    },
    Args,
};
use tokio::io::AsyncReadExt;

async fn get_hash(bytes: &[u8]) -> Result<()> {
    let comres = Command::new("openssl dgst -binary -sha256")
        .debug(true)
        .run_with_input_to_completion(bytes)
        .await
        .stack()?;
    comres.assert_success().stack()?;
    let comres = Command::new("openssl base64 -A")
        .debug(true)
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
        let file = FileOptions::read_to_string(
            args.genesis_path
                .as_deref()
                .stack_err(|| "need --genesis-path")?,
        )
        .await
        .stack()?;
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

    sh([format!(
        "cp {daemon_home}/cosmovisor/current/bin/onexd /logs/onexd"
    )])
    .await
    .stack()?;

    Ok(())
}
