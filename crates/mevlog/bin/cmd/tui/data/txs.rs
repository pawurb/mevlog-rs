use std::time::Duration;

use eyre::Result;
use mevlog::{cmds, misc::shared_init::CryoOpts};
use tokio::time::timeout;

use crate::cmd::tui::data::{LogJson, TransactionJson, conn_opts};

const CMD_TIMEOUT: Duration = Duration::from_secs(120);

#[hotpath::measure(future = true)]
pub(crate) async fn fetch_txs(blocks: &str, rpc_url: String) -> Result<Vec<TransactionJson>> {
    let conn = conn_opts(rpc_url);
    match timeout(
        CMD_TIMEOUT,
        cmds::block_txs::block_txs_typed(blocks, None, None, &conn, &CryoOpts::default()),
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
pub(crate) async fn fetch_txs_with_logs(
    blocks: &str,
    rpc_url: String,
) -> Result<Vec<TransactionJson>> {
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
        cmds::block_logs::block_logs_typed(blocks, None, &conn, &CryoOpts::default()),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => eyre::bail!("block-logs timed out after 120 seconds"),
    }
}
