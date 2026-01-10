use std::sync::Arc;

use eyre::Result;
use mevlog::misc::{rpc_capability::is_debug_trace_available, shared_init::init_provider};

#[derive(Debug, clap::Parser)]
pub struct DebugAvailableArgs {
    #[arg(long, help = "RPC URL to check for debug tracing support")]
    pub rpc_url: String,

    #[arg(long, help = "Timeout in milliseconds", default_value = "5000")]
    pub timeout_ms: u64,
}

impl DebugAvailableArgs {
    pub async fn run(&self) -> Result<()> {
        let provider = Arc::new(init_provider(&self.rpc_url).await?);
        let available = is_debug_trace_available(&provider, self.timeout_ms).await;
        println!("{}", available);
        Ok(())
    }
}
