use std::{sync::Arc, time::Duration};

use alloy::primitives::TxHash;
use eyre::Result;
use mevlog::cmds;
use mevlog::misc::rpc_capability::is_debug_trace_available;
use mevlog::misc::shared_init::{TraceMode, init_provider};
use mevlog::models::call_extract::CallExtract;
use mevlog::models::json::state_diff_json::StateDiffJson;
use tokio::time::timeout;

use crate::cmd::tui::data::{LogJson, TransactionJson, conn_opts};

const CMD_TIMEOUT: Duration = Duration::from_secs(120);

#[hotpath::measure(future = true)]
pub async fn fetch_txs(blocks: &str, rpc_url: String) -> Result<Vec<TransactionJson>> {
    let conn = conn_opts(rpc_url);
    match timeout(
        CMD_TIMEOUT,
        cmds::block_txs::block_txs_typed(blocks, None, None, &conn),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => eyre::bail!("block-txs timed out after 120 seconds"),
    }
}

/// Fetch the txs of a block together with all its logs, attaching each tx's logs
/// in memory. The block is indexed once by `block_txs`, so the follow-up
/// `block_logs` call is a local read (no extra RPC). Logs are grouped by
/// `tx_index`, which avoids the per-tx `tx-logs` RPC the popup used to make on
/// every selection change.
#[hotpath::measure(future = true)]
pub async fn fetch_txs_with_logs(blocks: &str, rpc_url: String) -> Result<Vec<TransactionJson>> {
    let mut txs = fetch_txs(blocks, rpc_url.clone()).await?;
    if txs.is_empty() {
        return Ok(txs);
    }

    // Pin the logs query to the concrete block number we just fetched so a "latest"
    // request can't race a newly produced block between the two calls.
    let block_number = txs[0].block_number;
    let logs = fetch_block_logs(&block_number.to_string(), rpc_url).await?;

    let mut by_tx_index: std::collections::HashMap<u64, Vec<LogJson>> =
        std::collections::HashMap::new();
    for log in logs {
        if let Some(tx_index) = log.tx_index {
            by_tx_index.entry(tx_index).or_default().push(log);
        }
    }
    for tx in &mut txs {
        if let Some(logs) = by_tx_index.remove(&tx.tx_index) {
            tx.logs = logs;
        }
    }

    Ok(txs)
}

#[hotpath::measure(future = true)]
async fn fetch_block_logs(blocks: &str, rpc_url: String) -> Result<Vec<LogJson>> {
    let conn = conn_opts(rpc_url);
    match timeout(
        CMD_TIMEOUT,
        cmds::block_logs::block_logs_typed(blocks, None, &conn),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => eyre::bail!("block-logs timed out after 120 seconds"),
    }
}

#[hotpath::measure(log = true, future = true)]
pub async fn detect_trace_mode(rpc_url: &str) -> TraceMode {
    let Ok(provider) = init_provider(rpc_url).await else {
        return TraceMode::Revm;
    };
    let provider = Arc::new(provider);
    if is_debug_trace_available(&provider, 5000).await {
        TraceMode::RPC
    } else {
        TraceMode::Revm
    }
}

#[hotpath::measure(log = true, future = true)]
pub async fn fetch_traces(
    tx_hash: &str,
    rpc_url: String,
    trace_mode: TraceMode,
) -> Result<Vec<CallExtract>> {
    let tx_hash: TxHash = tx_hash.parse()?;
    let conn = conn_opts(rpc_url);
    match timeout(
        CMD_TIMEOUT,
        cmds::evm_traces::evm_traces(tx_hash, Some(&trace_mode), &conn),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => eyre::bail!("evm-traces timed out after 120 seconds"),
    }
}

#[hotpath::measure(future = true)]
pub async fn fetch_tx_with_trace(
    tx_hash: &str,
    rpc_url: String,
    trace_mode: TraceMode,
) -> Result<TransactionJson> {
    let tx_hash: TxHash = tx_hash.parse()?;
    let conn = conn_opts(rpc_url);
    match timeout(
        CMD_TIMEOUT,
        cmds::tx::tx_typed(tx_hash, Some(&trace_mode), None, &conn),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => eyre::bail!("tx --evm-trace timed out after 120 seconds"),
    }
}

#[hotpath::measure(future = true)]
pub async fn fetch_state_diff(
    tx_hash: &str,
    rpc_url: String,
    trace_mode: TraceMode,
) -> Result<StateDiffJson> {
    let tx_hash: TxHash = tx_hash.parse()?;
    let conn = conn_opts(rpc_url);
    let state_diff = match timeout(
        CMD_TIMEOUT,
        cmds::state_diff::state_diff(tx_hash, Some(&trace_mode), &conn),
    )
    .await
    {
        Ok(res) => res?,
        Err(_) => eyre::bail!("evm-state-diff timed out after 120 seconds"),
    };
    Ok(StateDiffJson::from(&state_diff))
}
