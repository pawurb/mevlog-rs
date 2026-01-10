use std::{sync::Arc, time::Duration};

use alloy::{
    eips::BlockNumberOrTag,
    providers::{Provider, ext::DebugApi},
    rpc::types::trace::geth::{
        GethDebugBuiltInTracerType, GethDebugTracerType, GethDebugTracingOptions,
    },
};
use tokio::time::timeout;
use tracing::debug;

use crate::GenericProvider;

pub async fn is_debug_trace_available(provider: &Arc<GenericProvider>, timeout_ms: u64) -> bool {
    debug!("Fetching a recent block to get a test transaction hash");
    let Some(tx_hash) = get_test_tx_hash(provider).await else {
        debug!("Failed to get a test transaction hash");
        return false;
    };
    debug!(%tx_hash, "Using transaction for debug trace test");

    let tracing_opts = GethDebugTracingOptions::default().with_tracer(
        GethDebugTracerType::BuiltInTracer(GethDebugBuiltInTracerType::CallTracer),
    );

    debug!(%timeout_ms, "Calling debug_traceTransaction with CallTracer");
    let result = timeout(
        Duration::from_millis(timeout_ms),
        provider.debug_trace_transaction(tx_hash, tracing_opts),
    )
    .await;

    match &result {
        Ok(Ok(_)) => debug!("debug_traceTransaction succeeded"),
        Ok(Err(e)) => debug!(%e, "debug_traceTransaction failed"),
        Err(_) => debug!("debug_traceTransaction timed out"),
    }

    matches!(result, Ok(Ok(_)))
}

async fn get_test_tx_hash(provider: &Arc<GenericProvider>) -> Option<alloy::primitives::TxHash> {
    let latest = provider.get_block_number().await.ok()?;
    debug!(%latest, "Got latest block number");

    for offset in [100, 50, 10, 5, 1] {
        let block_num = latest.saturating_sub(offset);
        debug!(%block_num, "Trying to get transaction from block");

        let block = provider
            .get_block_by_number(BlockNumberOrTag::Number(block_num))
            .await
            .ok()??;

        if let Some(tx_hash) = block.transactions.hashes().next() {
            return Some(tx_hash);
        }
    }

    None
}
