use onomy_test_lib::{
    super_orchestrator::{
        docker::{Container, ContainerNetwork, Dockerfile},
        sh,
        stacked_errors::{Result, StackableErr},
    },
    Args, TIMEOUT,
};
pub mod contest;
mod get_key;
pub use get_key::*;

/// Useful for running simple container networks that have a standard format and
/// don't need extra build or volume arguments.
pub async fn container_runner(args: &Args, name_and_contents: &[(&str, &str)]) -> Result<()> {
    let logs_dir = "./tests/logs";
    let resources_dir = "./tests/resources";
    let dockerfiles_dir = "./tests/dockerfiles";
    let bin_entrypoint = &args.bin_name;
    let container_target = "x86_64-unknown-linux-gnu";

    // build internal runner
    sh([
        "cargo build --release --bin",
        bin_entrypoint,
        "--target",
        container_target,
    ])
    .await
    .stack()?;

    let mut containers = vec![];
    for (name, contents) in name_and_contents {
        containers.push(
            Container::new(name, Dockerfile::contents(contents))
                .external_entrypoint(
                    format!("./target/{container_target}/release/{bin_entrypoint}"),
                    ["--entry-name", name],
                )
                .await
                .stack()?,
        );
    }

    let mut cn =
        ContainerNetwork::new("test", containers, Some(dockerfiles_dir), true, logs_dir).stack()?;
    cn.add_common_volumes([(logs_dir, "/logs"), (resources_dir, "/resources")]);
    let uuid = cn.uuid_as_string();
    cn.add_common_entrypoint_args(["--uuid", &uuid]);
    cn.run_all(true).await.stack()?;
    cn.wait_with_timeout_all(true, TIMEOUT).await.stack()?;
    cn.terminate_all().await;
    Ok(())
}

pub const MODULE_ACCOUNTS: &[&str] = &[
    "onomy1fl48vsnmsdzcv85q5d2q4z5ajdha8yu306aegj",
    "onomy1tygms3xhhs3yv487phx3dw4a95jn7t7lm6pg7x",
    "onomy1vwr8z00ty7mqnk4dtchr9mn9j96nuh6wrlww93",
    "onomy10d07y265gmmuvt4z0w9aw880jnsr700jqr8n8k",
    "onomy1jv65s3grqf6v6jl3dp4t6c9t9rk99cd8a7s2c6",
    "onomy1m3h30wlvsf8llruxtpukdvsy0km2kum8jsnwk9",
    "onomy17xpfvakm2amg962yls6f84z3kell8c5l2chk6c",
    "onomy16n3lc7cywa68mg50qhp847034w88pntquhhcyk",
    "onomy1yl6hdjhmkf37639730gffanpzndzdpmh57zlxx",
    "onomy1ap0mh6xzfn8943urr84q6ae7zfnar48aptd4xg",
];
