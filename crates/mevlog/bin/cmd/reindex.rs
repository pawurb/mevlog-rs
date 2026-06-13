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
    batch_size: usize,
}

impl ReindexArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        if matches!(format, OutputFormat::Csv | OutputFormat::Table) {
            bail!("'csv' and 'table' formats are only supported by the query command");
        }

        let deps = init_deps(&self.conn_opts).await?;

        // Read the indexed range from the local store, then re-run indexing over
        // it: `index_block_range` only fetches blocks absent from the DB, so this
        // backfills the gaps inside `min_block..=max_block` without re-fetching
        // anything already present.
        let stats = db_info(&deps.txs).await?;
        let (Some(from), Some(to)) = (stats.min_block, stats.max_block) else {
            bail!("Txs DB has no indexed blocks; nothing to reindex");
        };

        if stats.missing_blocks == 0 {
            bail!(
                "No missing blocks in indexed range {}..={}; nothing to reindex",
                from,
                to
            );
        }

        let start_time = Instant::now();
        let (cached_blocks, new_blocks) =
            index_block_range(from, to, self.batch_size, &deps, &self.cryo_opts).await?;
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
