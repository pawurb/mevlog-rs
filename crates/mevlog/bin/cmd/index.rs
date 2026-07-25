use std::time::{Duration, Instant};

use alloy::{eips::BlockNumberOrTag, providers::Provider};
use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{
        indexing::index_block_range,
        purge::purge_old_blocks,
        reorg::{find_fork_point, first_parent_link_break, rollback_from},
    },
    misc::{
        args_parsing::BlocksRange,
        data_fetch::purge_cache_range,
        shared_init::{ConnOpts, CryoOpts, OutputFormat, SharedDeps, init_deps},
    },
    models::json::index_response::{IndexResponse, serialize_index_response},
};
use tracing::{info, warn};

#[derive(Debug, clap::Parser)]
pub struct IndexArgs {
    #[arg(
        short = 'b',
        long,
        help = "Block number or range to index (e.g., '22030899', 'latest', '22030800:22030900', '50:latest', '50:'). Required unless --live is set"
    )]
    blocks: Option<String>,

    #[command(flatten)]
    conn_opts: ConnOpts,

    #[command(flatten)]
    cryo_opts: CryoOpts,

    #[arg(long, help = "Get N-offset latest block")]
    latest_offset: Option<u64>,

    #[arg(long, help = "Maximum allowed block range size")]
    max_range: Option<u64>,

    #[arg(
        long,
        help = "Batch size for data fetching (default: 100)",
        default_value = "100"
    )]
    batch_size: std::num::NonZeroUsize,

    #[arg(
        long,
        help = "Keep watching for new blocks and index them as they arrive"
    )]
    live: bool,

    #[arg(
        long,
        help = "Polling interval in milliseconds when --live is set (default: 3000)",
        default_value = "3000"
    )]
    poll_interval_ms: u64,

    #[arg(
        long,
        help = "With --live: after each indexing round, delete data older than this many blocks behind the newest indexed block"
    )]
    keep: Option<u64>,

    #[arg(
        long,
        help = "With --live: maximum number of blocks scanned below the local tip when checking for a chain reorg (default: 64)",
        default_value = "64"
    )]
    max_reorg_depth: std::num::NonZeroU64,
}

impl IndexArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        if !self.live && self.blocks.is_none() {
            bail!("--blocks is required unless --live is set");
        }

        if self.keep.is_some() && !self.live {
            bail!("--keep requires --live; use the purge-db command for one-off pruning");
        }

        if self.keep == Some(0) {
            bail!("--keep must be at least 1; use 'purge-db --keep 0' to wipe the DB");
        }

        if matches!(
            format,
            OutputFormat::Csv | OutputFormat::Table | OutputFormat::Html
        ) {
            bail!("'csv', 'table' and 'html' formats are only supported by the query command");
        }

        let deps = init_deps(&self.conn_opts).await?;

        // Backfill the requested range (in both normal and live mode).
        let backfilled_to = match &self.blocks {
            Some(blocks) => {
                let range =
                    BlocksRange::from_str(blocks, &deps.provider, self.latest_offset).await?;

                if let Some(max_range) = self.max_range {
                    let range_size = range.size();
                    if range_size > max_range {
                        bail!(
                            "Block range size {} exceeds maximum allowed range of {}",
                            range_size,
                            max_range
                        );
                    }
                }

                let start_time = Instant::now();
                let (cached_blocks, new_blocks) = index_block_range(
                    range.from,
                    range.to,
                    self.batch_size.get(),
                    &deps,
                    &self.cryo_opts,
                )
                .await?;
                let duration_ns = start_time.elapsed().as_nanos() as u64;

                if self.live {
                    info!(
                        "Backfilled blocks {}..={} ({} new, {} cached)",
                        range.from, range.to, new_blocks, cached_blocks
                    );
                } else {
                    let chain = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
                    let resp = IndexResponse::new(
                        blocks.clone(),
                        range.from,
                        range.to,
                        cached_blocks,
                        new_blocks,
                        duration_ns,
                        chain,
                    );
                    let pretty = !matches!(format, OutputFormat::Json);
                    println!("{}", serialize_index_response(&resp, pretty)?);
                }

                Some(range.to)
            }
            None => None,
        };

        if !self.live {
            return Ok(());
        }

        // Live mode: poll for new blocks and index them as they arrive.
        let mut last_indexed = match backfilled_to {
            Some(to) => to,
            None => {
                // No backfill range given: start from the current latest block.
                let latest = deps.provider.get_block_number().await?;
                let (cached_blocks, new_blocks) = index_block_range(
                    latest,
                    latest,
                    self.batch_size.get(),
                    &deps,
                    &self.cryo_opts,
                )
                .await?;
                info!(
                    "Indexed latest block {} ({} new, {} cached)",
                    latest, new_blocks, cached_blocks
                );
                latest
            }
        };

        // One-time purge after the backfill, instead of waiting for the first
        // new block.
        if let Some(keep) = self.keep {
            purge_and_log(keep, &deps.txs).await?;
        }

        info!(
            "Watching for new blocks (poll every {}ms)",
            self.poll_interval_ms
        );
        let poll = Duration::from_millis(self.poll_interval_ms);

        loop {
            let latest = deps.provider.get_block_number().await?;

            // The head moved (forward, or backward after a reorg): verify the
            // local tip is still canonical before extending it.
            if latest != last_indexed
                && let Some(fork_point) =
                    detect_fork_point(&deps, last_indexed, self.max_reorg_depth.get()).await?
            {
                rollback_and_log(fork_point + 1, last_indexed, &deps).await?;
                last_indexed = fork_point;
            }

            if latest > last_indexed {
                let from = last_indexed + 1;
                let start_time = Instant::now();
                let (cached_blocks, new_blocks) =
                    index_block_range(from, latest, self.batch_size.get(), &deps, &self.cryo_opts)
                        .await?;
                info!(
                    "Indexed blocks {}..={} ({} new, {} cached) in {:.2?}",
                    from,
                    latest,
                    new_blocks,
                    cached_blocks,
                    start_time.elapsed()
                );

                // A reorg racing the fetch can land a mix of pre- and
                // post-reorg blocks in one round; broken parent links inside
                // the freshly indexed range expose it without extra RPC calls.
                if let Some(break_block) = first_parent_link_break(from, latest, &deps.txs).await? {
                    rollback_and_log(break_block, latest, &deps).await?;
                    last_indexed = break_block - 1;
                } else {
                    last_indexed = latest;
                }

                if let Some(keep) = self.keep {
                    purge_and_log(keep, &deps.txs).await?;
                }
            }
            tokio::time::sleep(poll).await;
        }
    }
}

/// Compares locally stored block hashes against the canonical chain, walking
/// back from `tip`. `None` means the local tip is canonical; `Some(fork_point)`
/// means every block above `fork_point` is stale.
async fn detect_fork_point(deps: &SharedDeps, tip: u64, max_depth: u64) -> Result<Option<u64>> {
    let provider = deps.provider.clone();
    find_fork_point(tip, max_depth, &deps.txs, move |block_number| {
        let provider = provider.clone();
        async move {
            let block = provider
                .get_block_by_number(BlockNumberOrTag::Number(block_number))
                .await?;
            Ok(block.map(|b| b.header.hash))
        }
    })
    .await
}

/// Deletes indexed data for blocks `first_stale..=tip` plus any cryo parquet
/// cache overlapping them (a refetch must not reuse pre-reorg rows).
async fn rollback_and_log(first_stale: u64, tip: u64, deps: &SharedDeps) -> Result<()> {
    let stats = rollback_from(first_stale, &deps.txs).await?;
    purge_cache_range(&deps.chain, first_stale, tip);
    warn!(
        "Reorg detected: rolled back blocks {}..={} ({} blocks, {} txs, {} logs)",
        first_stale, tip, stats.blocks, stats.transactions, stats.logs
    );
    Ok(())
}

async fn purge_and_log(keep: u64, conn: &sqlx::SqlitePool) -> Result<()> {
    let stats = purge_old_blocks(keep, false, conn).await?;
    if stats.purged_blocks > 0 {
        info!(
            "Purged {} blocks below {} ({} txs, {} logs)",
            stats.purged_blocks,
            stats.cutoff_block.unwrap_or_default(),
            stats.purged_transactions,
            stats.purged_logs
        );
    }
    Ok(())
}
