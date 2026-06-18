use std::time::Instant;

use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{indexing::index_block_range, info::db_info},
    misc::shared_init::{ConnOpts, CryoOpts, OutputFormat, init_deps},
    models::json::index_response::{IndexResponse, serialize_index_response},
};

#[derive(Debug, clap::Parser)]
pub struct ReindexArgs {
    #[command(flatten)]
    conn_opts: ConnOpts,

    #[command(flatten)]
    cryo_opts: CryoOpts,

    #[arg(
        long,
        help = "Batch size for data fetching (default: 100)",
        default_value = "100"
    )]
    batch_size: std::num::NonZeroUsize,

    #[arg(
        long,
        help = "Only reindex blocks within this distance of the newest indexed block; older gaps are left alone. Defaults to the whole indexed range. Mirror the purge --keep window to keep reindex from backfilling blocks that purge would immediately drop"
    )]
    keep: Option<std::num::NonZeroU64>,
}

impl ReindexArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        if matches!(format, OutputFormat::Csv | OutputFormat::Table) {
            bail!("'csv' and 'table' formats are only supported by the query command");
        }

        let deps = init_deps(&self.conn_opts).await?;

        // Re-run indexing over the stored range; `index_block_range` only fetches
        // blocks absent from the DB, so this backfills the gaps. A contiguous
        // range is a no-op (`new_blocks = 0`), keeping it safe to run on a schedule.
        let stats = db_info(&deps.txs).await?;
        let (Some(min_block), Some(to)) = (stats.min_block, stats.max_block) else {
            bail!("Txs DB has no indexed blocks; nothing to reindex");
        };

        // Clamp the floor to a `keep`-sized window behind the newest block so a
        // single stray old block (e.g. one indexed by an on-demand `block-txs`
        // web request) can't make reindex backfill the entire gap up to the tip.
        // Mirror the purge --keep window: anything below it would be purged anyway.
        let from = match self.keep {
            Some(keep) => min_block.max(to.saturating_add(1).saturating_sub(keep.get())),
            None => min_block,
        };

        let start_time = Instant::now();
        let (cached_blocks, new_blocks) =
            index_block_range(from, to, self.batch_size.get(), &deps, &self.cryo_opts).await?;
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        let chain = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        let resp = IndexResponse::new(
            format!("{from}:{to}"),
            from,
            to,
            cached_blocks,
            new_blocks,
            duration_ns,
            chain,
        );
        let pretty = !matches!(format, OutputFormat::Json);
        println!("{}", serialize_index_response(&resp, pretty)?);

        Ok(())
    }
}
