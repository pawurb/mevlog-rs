use std::time::{Duration, Instant};

use eyre::{Result, bail, eyre};

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::{indexing::index_block_range, raw_query::run_raw_query_async},
    misc::{
        args_parsing::BlocksRange,
        shared_init::{ConnOpts, CryoOpts, SharedOpts, init_deps},
        sql_macros::substitute_sql_macros,
        tx_tracing::backfill_coinbase_transfers,
        utils::get_native_token_price,
    },
    models::json::query_response::{QueryOutcome, QueryParams},
};

/// Collects all txs within a block range into the local store and runs the
/// given read-only SQL against it.
#[allow(clippy::too_many_arguments)]
pub async fn query(
    blocks: Option<&str>,
    latest_offset: Option<u64>,
    max_range: Option<u64>,
    max_rows: Option<usize>,
    batch_size: usize,
    skip_index: bool,
    latest_block: Option<u64>,
    sql: &str,
    shared_opts: &SharedOpts,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
    timeout_ms: Option<u64>,
) -> Result<QueryOutcome> {
    let start_time = Instant::now();
    let deadline = timeout_ms
        .filter(|&ms| ms > 0)
        .map(|ms| start_time + Duration::from_millis(ms));

    // The pre-SQL phase (deps + RPC, indexing, price/macro/ENS lookups) is all
    // async and cancels cleanly when its future is dropped, so the whole phase
    // shares one wall-clock budget via tokio::time::timeout_at.
    let prep = async {
        let deps = init_deps(conn_opts).await?;

        let native_token_price =
            get_native_token_price(&deps.chain, &deps.provider, shared_opts.native_token_price)
                .await?;

        // With --skip-index the local store is queried as-is: no block range
        // resolution (so no RPC for 'latest'), no fetching, no backfill.
        let (cached_blocks, new_blocks) = if skip_index {
            if blocks.is_some() {
                bail!("'--blocks' and '--skip-index' are mutually exclusive");
            }
            (0, 0)
        } else {
            let Some(blocks) = blocks else {
                bail!("'--blocks' is required unless --skip-index is enabled");
            };
            let block_range = BlocksRange::from_str(blocks, &deps.provider, latest_offset).await?;

            if let Some(max_range) = max_range {
                let range_size = block_range.size();
                if range_size > max_range {
                    bail!(
                        "Block range size {} exceeds maximum allowed range of {}",
                        range_size,
                        max_range
                    );
                }
            }

            // Only fetch blocks that are not already in the local store. Indexed
            // blocks (including empty ones, tracked by the `blocks` table) are skipped.
            let counts = index_block_range(
                block_range.from,
                block_range.to,
                batch_size,
                &deps,
                cryo_opts,
            )
            .await?;

            // Backfill direct coinbase payments for any untraced txs in range. Runs
            // over the local store, so it also covers blocks indexed earlier without
            // --evm-trace.
            if let Some(mode) = &shared_opts.evm_trace {
                backfill_coinbase_transfers(
                    block_range.from,
                    block_range.to,
                    mode,
                    &deps.provider,
                    &deps.chain,
                    &deps.rpc_url,
                    &deps.txs,
                )
                .await?;
            }

            counts
        };

        let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        chain_info.native_token_price = native_token_price;

        let sql = substitute_sql_macros(
            sql,
            &deps.provider,
            deps.chain.chain_id,
            native_token_price,
            latest_block,
        )
        .await?;

        Ok::<_, eyre::Report>((
            deps.txs_read_path.clone(),
            chain_info,
            cached_blocks,
            new_blocks,
            sql,
            deps.custom_table_names(),
        ))
    };

    let (txs_read_path, chain_info, cached_blocks, new_blocks, sql, custom_tables) = match deadline
    {
        Some(dl) => tokio::time::timeout_at(tokio::time::Instant::from_std(dl), prep)
            .await
            .map_err(|_| eyre!("Query timed out after {}ms", timeout_ms.unwrap()))??,
        None => prep.await?,
    };

    // The SQL runs in a blocking task a dropped future can't cancel, so the
    // remaining budget is enforced inside SQLite via its progress handler.
    let remaining = deadline.map(|dl| dl.saturating_duration_since(Instant::now()));
    let result = run_raw_query_async(
        sql.clone(),
        txs_read_path,
        max_rows,
        remaining,
        custom_tables,
    )
    .await?;

    let duration_ns = start_time.elapsed().as_nanos() as u64;

    Ok(QueryOutcome {
        columns: result.columns,
        rows: result.rows,
        cached_blocks,
        new_blocks,
        duration_ns,
        chain: chain_info,
        query: QueryParams {
            blocks: blocks.map(str::to_string),
            sql: Some(sql),
            evm_trace: shared_opts.evm_trace.clone(),
        },
    })
}
