use std::time::Instant;

use alloy::{network::ReceiptResponse, primitives::TxHash, providers::Provider};
use eyre::Result;

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::logs_display_query, indexing::index_block_range,
        raw_query::run_raw_query_async,
    },
    misc::shared_init::{ConnOpts, CryoOpts, init_deps},
    models::json::query_response::{QueryOutcome, QueryParams},
};

/// Indexes the block holding `tx_hash` and returns that transaction's logs,
/// shaped for display.
pub async fn tx_logs(
    tx_hash: TxHash,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<QueryOutcome> {
    let deps = init_deps(conn_opts).await?;

    let receipt = deps
        .provider
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction {} not found", tx_hash))?;
    let block_number = receipt
        .block_number()
        .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", tx_hash))?;
    let tx_index = receipt
        .transaction_index()
        .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", tx_hash))?;

    let start_time = Instant::now();

    let (cached_blocks, new_blocks) =
        index_block_range(block_number, block_number, 1, &deps, cryo_opts).await?;

    let chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
    let duration_ns = start_time.elapsed().as_nanos() as u64;

    // The logs SELECT has no macros (no USD columns), so it runs as-is.
    let sql = logs_display_query(&format!(
        "block_number = {block_number} AND tx_index = {tx_index}"
    ));
    let result = run_raw_query_async(sql.clone(), deps.txs_read_path.clone(), None).await?;

    Ok(QueryOutcome {
        columns: result.columns,
        rows: result.rows,
        cached_blocks,
        new_blocks,
        duration_ns,
        chain: chain_info,
        query: QueryParams {
            blocks: Some(block_number.to_string()),
            sql: Some(sql),
            evm_trace: None,
            evm_calls: false,
            evm_ops: false,
            evm_state_diff: false,
        },
    })
}
