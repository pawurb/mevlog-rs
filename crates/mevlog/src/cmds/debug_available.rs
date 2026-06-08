use std::sync::Arc;

use eyre::Result;

use crate::misc::{rpc_capability::is_debug_trace_available, shared_init::init_provider};

/// Returns whether the given RPC endpoint supports `debug_traceTransaction`.
pub async fn debug_available(rpc_url: &str, timeout_ms: u64) -> Result<bool> {
    let provider = Arc::new(init_provider(rpc_url).await?);
    Ok(is_debug_trace_available(&provider, timeout_ms).await)
}
