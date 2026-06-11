use std::time::Instant;

use eyre::{Result, bail};

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::block_display_query, indexing::index_block_range, raw_query::run_raw_query,
    },
    misc::{
        args_parsing::BlocksRange,
        shared_init::{ConnOpts, CryoOpts, init_deps},
    },
    models::json::query_response::{QueryOutcome, QueryParams},
};

/// Indexes a single block and returns its metadata, shaped for display.
pub async fn block(
    block: &str,
    latest_offset: Option<u64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<QueryOutcome> {
    let deps = init_deps(conn_opts).await?;

    let range = BlocksRange::from_str(block, &deps.provider, latest_offset).await?;
    if range.from != range.to {
        bail!("block expects a single block number or 'latest', not a range");
    }
    let block_number = range.from;

    let start_time = Instant::now();

    let (cached_blocks, new_blocks) =
        index_block_range(block_number, block_number, 1, &deps, cryo_opts).await?;

    let chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
    let duration_ns = start_time.elapsed().as_nanos() as u64;

    let sql = block_display_query(&format!("block_number = {block_number}"));
    let result = run_raw_query(&sql, &deps.txs_read_path, None)?;
    if result.rows.is_empty() {
        bail!("Block {block_number} not found in local store");
    }

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
