use common::{container_runner, dockerfile_onexd};
use onomy_test_lib::{
    onomy_std_init,
    super_orchestrator::{
        stacked_errors::{Error, Result, StackableErr},
        Command, FileOptions,
    },
    Args, TIMEOUT,
};
use tokio::{io::AsyncReadExt, time::sleep};

const FILE: &str =
    include_str!("./../../../../environments/testnet/onex-testnet-2/partial-genesis.json");

#[tokio::main]
async fn main() -> Result<()> {
    let args = onomy_std_init()?;

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
    let mut out = comres.stdout_as_utf8().stack()?.to_owned();
    // not sure why this needs to be done
    out.pop().unwrap();
    out.push('=');
    println!("{out}");

    if let Some(ref s) = args.entry_name {
        match s.as_str() {
            "runner" => runner(&args).await,
            _ => Err(Error::from(format!("entry_name \"{s}\" is not recognized"))),
        }
    } else {
        container_runner(&args, &[("runner", &dockerfile_onexd())])
            .await
            .stack()
    }
}

async fn runner(args: &Args) -> Result<()> {
    let daemon_home = &args.daemon_home.clone().stack()?;

    sleep(TIMEOUT).await;
    let mut file = FileOptions::read(&format!("{daemon_home}/cosmovisor/current/bin/onexd"))
        .acquire_file()
        .await
        .stack()?;
    let mut contents = vec![];
    file.read_to_end(&mut contents).await.stack()?;

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
    let mut out = comres.stdout_as_utf8().stack()?.to_owned();
    // not sure why this needs to be done
    out.pop().unwrap();
    out.push('=');
    println!("{out}");

    Ok(())
}
