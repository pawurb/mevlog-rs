use eyre::Result;
use futures_util::{StreamExt, stream};
use tracing::info;

use crate::{
    db::txs::{
        custom_tables,
        models::{block::Block, log::Log, transaction::Transaction},
    },
    misc::{
        data_fetch::{fetch_blocks_batch, prune_indexed_cache},
        shared_init::{CryoOpts, SharedDeps},
    },
};

/// Indexes every block in `from..=to` that is not already in the local store,
/// fetching missing blocks in contiguous runs of `batch_size` and persisting
/// their txs, logs, and block rows. Returns `(cached_blocks, new_blocks)`.
///
/// Backfill proceeds newest-block-first, so the most recent blocks become
/// queryable first and an interrupted backfill leaves the gap at the bottom of
/// the range. This also keeps a live indexer, which appends forward from the
/// newest indexed block, from colliding with backfill on the same blocks.
///
/// Blocks are persisted last in each chunk: a `blocks` row marks a block as
/// indexed, so a block is only flagged once its txs and logs have landed. Every
/// block in the chunk (including empty ones) yields a block row, so empty
/// blocks are still recorded as indexed.
///
/// When `deps.rpc_urls` holds more than one endpoint (multiple `--rpc-url`
/// flags), chunks are fetched concurrently with one cryo process per endpoint
/// (round-robin), while persistence stays single-writer. With a single endpoint
/// the pool collapses to a sequential fetch-then-persist loop, preserving the
/// original newest-first ordering and behavior.
pub async fn index_block_range(
    from: u64,
    to: u64,
    batch_size: usize,
    deps: &SharedDeps,
    cryo_opts: &CryoOpts,
) -> Result<(u64, u64)> {
    let missing = Block::missing_blocks(from, to, &deps.txs).await?;

    let range_size = to.saturating_sub(from) + 1;
    let new_blocks = missing.len() as u64;
    let cached_blocks = range_size.saturating_sub(new_blocks);

    info!(
        "Blocks: {} cached, {} to fetch ({} total)",
        cached_blocks, new_blocks, range_size
    );

    let ranges = contiguous_ranges(&missing);

    // Flatten the gaps into batch-sized chunks, newest-first (ranges descending,
    // chunks descending within a run). Blocks inside a chunk stay ascending.
    let mut chunks: Vec<Vec<u64>> = vec![];
    for (run_start, run_end) in ranges.into_iter().rev() {
        let run_blocks: Vec<u64> = (run_start..=run_end).collect();
        for chunk in run_blocks.chunks(batch_size).rev() {
            chunks.push(chunk.to_vec());
        }
    }

    let total_batches = chunks.len();

    // One cryo process per endpoint; a single endpoint yields concurrency 1,
    // i.e. the original sequential fetch-then-persist behavior.
    let rpc_urls = &deps.rpc_urls;
    let workers = rpc_urls.len();

    let mut fetch_stream = stream::iter(chunks.into_iter().enumerate().map(|(i, chunk)| {
        let rpc_url = rpc_urls[i % workers].clone();
        async move {
            let start_block = *chunk.first().unwrap();
            let end_block = *chunk.last().unwrap();
            let batch_data = fetch_blocks_batch(
                start_block,
                end_block,
                &rpc_url,
                &deps.chain,
                &deps.sqlite,
                cryo_opts,
            )
            .await?;
            Ok::<_, eyre::Error>((chunk, start_block, end_block, batch_data))
        }
    }))
    .buffer_unordered(workers);

    let mut batch_idx = 0;
    while let Some(result) = fetch_stream.next().await {
        let (chunk, start_block, end_block, batch_data) = result?;

        batch_idx += 1;
        info!(
            "Indexing blocks {}-{} (batch {}/{})",
            start_block, end_block, batch_idx, total_batches
        );

        let mut chunk_txs: Vec<Transaction> = vec![];
        for &block_number in &chunk {
            if let Some(txs) = batch_data.txs_by_block.get(&block_number) {
                chunk_txs.extend(txs.iter().cloned());
            }
        }

        let mut chunk_logs: Vec<Log> = vec![];
        for &block_number in &chunk {
            if let Some(logs) = batch_data.logs_by_block.get(&block_number) {
                chunk_logs.extend(logs.iter().cloned());
            }
        }

        let mut chunk_blocks: Vec<Block> = vec![];
        for &block_number in &chunk {
            if let Some(block) = batch_data.blocks_by_block.get(&block_number) {
                chunk_blocks.push(block.clone());
            }
        }

        Log::save_batch(&chunk_logs, &deps.txs).await?;
        // Custom tables derive from the logs rows just written; populating
        // here keeps decoding in SQL with no second decode path.
        custom_tables::populate_range(&deps.custom_tables, start_block, end_block, &deps.txs)
            .await?;
        Transaction::save_batch(&chunk_txs, &deps.txs).await?;
        Block::save_batch(&chunk_blocks, &deps.txs).await?;
    }

    // Drop cryo parquet now fully captured in the txs DB; missing_blocks is the
    // source of truth, so cache for indexed blocks is never read again.
    let pruned = prune_indexed_cache(&deps.chain, &deps.txs, from, to).await?;
    if pruned > 0 {
        info!("Pruned {} cached parquet file(s)", pruned);
    }

    Ok((cached_blocks, new_blocks))
}

/// Collapses a sorted, deduplicated list of block numbers into contiguous
/// `(start, end)` inclusive ranges so each gap is fetched as a single batch.
fn contiguous_ranges(blocks: &[u64]) -> Vec<(u64, u64)> {
    let mut ranges: Vec<(u64, u64)> = vec![];

    for &block in blocks {
        match ranges.last_mut() {
            Some(last) if block == last.1 + 1 => last.1 = block,
            _ => ranges.push((block, block)),
        }
    }

    ranges
}
