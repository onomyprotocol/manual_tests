use onomy_test_lib::dockerfiles::onomy_std_cosmos_daemon_with_arbitrary;

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
