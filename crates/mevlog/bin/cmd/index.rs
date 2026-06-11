use std::time::{Duration, Instant};

use alloy::providers::Provider;
use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{indexing::index_block_range, purge::purge_old_blocks},
    misc::{
        args_parsing::BlocksRange,
        shared_init::{ConnOpts, OutputFormat, init_deps},
    },
    models::json::index_response::{IndexResponse, serialize_index_response},
};
use tracing::info;

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

    #[arg(long, help = "Get N-offset latest block")]
    latest_offset: Option<u64>,

    #[arg(long, help = "Maximum allowed block range size")]
    max_range: Option<u64>,

    #[arg(
        long,
        help = "Batch size for data fetching (default: 100)",
        default_value = "100"
    )]
    batch_size: usize,

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

        if matches!(format, OutputFormat::Csv | OutputFormat::Table) {
            bail!("'csv' and 'table' formats are only supported by the query command");
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
                let (cached_blocks, new_blocks) =
                    index_block_range(range.from, range.to, self.batch_size, &deps).await?;
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
                let (cached_blocks, new_blocks) =
                    index_block_range(latest, latest, self.batch_size, &deps).await?;
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
            if latest > last_indexed {
                let from = last_indexed + 1;
                let start_time = Instant::now();
                let (cached_blocks, new_blocks) =
                    index_block_range(from, latest, self.batch_size, &deps).await?;
                info!(
                    "Indexed blocks {}..={} ({} new, {} cached) in {:.2?}",
                    from,
                    latest,
                    new_blocks,
                    cached_blocks,
                    start_time.elapsed()
                );
                last_indexed = latest;

                if let Some(keep) = self.keep {
                    purge_and_log(keep, &deps.txs).await?;
                }
            }
            tokio::time::sleep(poll).await;
        }
    }
}

async fn purge_and_log(keep: u64, conn: &sqlx::SqlitePool) -> Result<()> {
    let stats = purge_old_blocks(keep, conn).await?;
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
