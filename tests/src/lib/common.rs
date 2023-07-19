use onomy_test_lib::{
    dockerfiles::onomy_std_cosmos_daemon_with_arbitrary,
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::Result,
    },
    Args, TIMEOUT,
};

#[rustfmt::skip]
const DOWNLOAD_ONOMYD: &str = r#"ADD https://github.com/onomyprotocol/onomy/releases/download/$DAEMON_VERSION/onomyd $DAEMON_HOME/cosmovisor/genesis/$DAEMON_VERSION/bin/onomyd"#;

pub fn dockerfile_onomyd() -> String {
    onomy_std_cosmos_daemon_with_arbitrary("onomyd", ".onomy", "v1.1.1", DOWNLOAD_ONOMYD)
}

#[rustfmt::skip]
const DOWNLOAD_HAVEND: &str = r#"ADD https://github.com/onomyprotocol/multiverse/releases/download/$DAEMON_VERSION/havend $DAEMON_HOME/cosmovisor/genesis/$DAEMON_VERSION/bin/havend"#;

pub fn dockerfile_havend() -> String {
    onomy_std_cosmos_daemon_with_arbitrary(
        "havend",
        ".onomy_haven",
        "v0.1.0-haven",
        DOWNLOAD_HAVEND,
    )
}

/// Useful for running simple container networks that have a standard format and
/// don't need extra build or volume arguments.
pub async fn container_runner(args: &Args, name_and_contents: &[(&str, &str)]) -> Result<()> {
    let logs_dir = "./tests/logs";
    let dockerfiles_dir = "./tests/dockerfiles";
    let bin_entrypoint = &args.bin_name;
    let container_target = "x86_64-unknown-linux-gnu";

    // build internal runner
    sh("cargo build --release --bin", &[
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await?;

    let mut cn = ContainerNetwork::new(
        "test",
        name_and_contents
            .iter()
            .map(|(name, contents)| {
                Container::new(
                    name,
                    Dockerfile::Contents(contents.to_string()),
                    Some(&format!(
                        "./target/{container_target}/release/{bin_entrypoint}"
                    )),
                    &["--entry-name", name],
                )
            })
            .collect(),
        Some(dockerfiles_dir),
        true,
        logs_dir,
    )?
    .add_common_volumes(&[(logs_dir, "/logs")]);
    cn.run_all(true).await?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.unwrap();
    Ok(())
}
