use std::time::Instant;

use eyre::{Result, bail};

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::block_logs_display_query, indexing::index_block_range,
        raw_query::run_raw_query_async,
    },
    misc::{
        args_parsing::{BlocksRange, get_latest_block},
        shared_init::{ConnOpts, CryoOpts, init_deps},
    },
    models::json::{
        log_json::LogJson,
        query_response::{QueryOutcome, QueryParams},
    },
};

/// Indexes a single block and returns all of its logs, shaped for display.
pub async fn block_logs(
    block: &str,
    latest_offset: Option<u64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<QueryOutcome> {
    let deps = init_deps(conn_opts).await?;

    let range = BlocksRange::from_str(block, &deps.provider, latest_offset).await?;
    if range.from != range.to {
        bail!("block-logs expects a single block number or 'latest', not a range");
    }
    let block_number = range.from;

    // 'latest' already resolved the chain head; only a numeric arg needs a fetch.
    let latest_block = if block == "latest" {
        Some(range.to)
    } else {
        Some(get_latest_block(&deps.provider, latest_offset).await?)
    };

    let start_time = Instant::now();

    let (cached_blocks, new_blocks) =
        index_block_range(block_number, block_number, 1, &deps, cryo_opts).await?;

    let chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
    let duration_ns = start_time.elapsed().as_nanos() as u64;

    // The logs SELECT has no macros (no USD columns), so it runs as-is.
    let sql = block_logs_display_query(&format!("block_number = {block_number}"));
    let result = run_raw_query_async(
        sql.clone(),
        deps.txs_read_path.clone(),
        None,
        None,
        deps.custom_table_names(),
    )
    .await?;

    Ok(QueryOutcome {
        columns: result.columns,
        rows: result.rows,
        cached_blocks,
        new_blocks,
        latest_block,
        duration_ns,
        chain: chain_info,
        query: QueryParams {
            blocks: Some(block_number.to_string()),
            sql: Some(sql),
            evm_trace: None,
        },
    })
}

/// Typed convenience wrapper for in-process callers (the TUI): returns the
/// block's logs as concrete [`LogJson`] values.
pub async fn block_logs_typed(
    block: &str,
    latest_offset: Option<u64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<Vec<LogJson>> {
    block_logs(block, latest_offset, conn_opts, cryo_opts)
        .await?
        .rows_as()
}
