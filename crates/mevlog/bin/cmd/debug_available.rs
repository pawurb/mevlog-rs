use eyre::Result;
use mevlog::cmds;

#[derive(Debug, clap::Parser)]
pub struct DebugAvailableArgs {
    #[arg(long, help = "RPC URL to check for debug tracing support")]
    pub rpc_url: String,

    #[arg(long, help = "Timeout in milliseconds", default_value = "5000")]
    pub timeout_ms: u64,
}

impl DebugAvailableArgs {
    pub async fn run(&self) -> Result<()> {
        let available =
            cmds::debug_available::debug_available(&self.rpc_url, self.timeout_ms).await?;
        println!("{}", available);
        Ok(())
    }
}
