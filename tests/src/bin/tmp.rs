use onomy_test_lib::{
    onomy_std_init,
    super_orchestrator::stacked_errors::{Result, StackableErr},
};
#[tokio::main]
async fn main() -> Result<()> {
    let _args = onomy_std_init().stack()?;

    Ok(())
}
