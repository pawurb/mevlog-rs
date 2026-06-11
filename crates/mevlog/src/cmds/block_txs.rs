use std::time::Instant;

use eyre::{Result, bail};

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::tx_display_query, indexing::index_block_range, raw_query::run_raw_query,
    },
    misc::{
        args_parsing::BlocksRange,
        shared_init::{ConnOpts, CryoOpts, init_deps},
        sql_macros::{NATIVE_TOKEN_PRICE_MACRO, substitute_sql_macros},
        utils::get_native_token_price,
    },
    models::json::{
        query_response::{QueryOutcome, QueryParams},
        transaction_json::TransactionJson,
    },
};

/// Indexes a single block and returns its transactions, shaped for display.
pub async fn block_txs(
    block: &str,
    latest_offset: Option<u64>,
    native_token_price: Option<f64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<QueryOutcome> {
    let deps = init_deps(conn_opts).await?;

    let native_token_price =
        get_native_token_price(&deps.chain, &deps.provider, native_token_price).await?;

    let range = BlocksRange::from_str(block, &deps.provider, latest_offset).await?;
    if range.from != range.to {
        bail!("block-txs expects a single block number or 'latest', not a range");
    }
    let block_number = range.from;

    let start_time = Instant::now();

    let (cached_blocks, new_blocks) =
        index_block_range(block_number, block_number, 1, &deps, cryo_opts).await?;

    let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
    chain_info.native_token_price = native_token_price;
    let duration_ns = start_time.elapsed().as_nanos() as u64;

    let sql = tx_display_query(&format!("block_number = {block_number}"));
    // Without a price the macro can't resolve, so substitute NULL; the USD
    // columns then render as null rather than erroring.
    let sql = if native_token_price.is_some() {
        substitute_sql_macros(
            &sql,
            &deps.provider,
            deps.chain.chain_id,
            native_token_price,
        )
        .await?
    } else {
        sql.replace(NATIVE_TOKEN_PRICE_MACRO, "NULL")
    };

    let result = run_raw_query(&sql, &deps.txs_read_path, None)?;

    Ok(QueryOutcome {
        columns: result.columns,
        rows: result.rows,
        cached_blocks,
        new_blocks,
        duration_ns,
        chain: chain_info,
        query: QueryParams {
            blocks: block_number.to_string(),
            sql: Some(sql),
            evm_trace: None,
            evm_calls: false,
            evm_ops: false,
            evm_state_diff: false,
        },
    })
}

/// Typed convenience wrapper for in-process callers (the TUI): returns the
/// block's transactions as concrete [`TransactionJson`] values.
pub async fn block_txs_typed(
    block: &str,
    latest_offset: Option<u64>,
    native_token_price: Option<f64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<Vec<TransactionJson>> {
    block_txs(
        block,
        latest_offset,
        native_token_price,
        conn_opts,
        cryo_opts,
    )
    .await?
    .rows_as()
}
