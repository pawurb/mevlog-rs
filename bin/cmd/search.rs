use alloy::providers::Provider;
use eyre::Result;
use mevlog::misc::args_parsing::BlocksRange;
use mevlog::misc::ens_utils::ENSLookup;
use mevlog::misc::shared_init::{init_deps, ConnOpts};
use mevlog::misc::utils::SEPARATORER;
use mevlog::models::mev_block::process_block;
use mevlog::models::txs_filter::{SharedFilterOpts, TxsFilter};

#[derive(Debug, clap::Parser)]
pub struct SearchArgs {
    #[arg(short = 'b', long, help_heading = "Block number or range to filter by (e.g., '22030899', 'latest', '22030800:22030900' '50:latest', '50:'", num_args(1..))]
    blocks: String,

    #[command(flatten)]
    filter: SharedFilterOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl SearchArgs {
    pub async fn run(&self) -> Result<()> {
        let shared_deps = init_deps(&self.conn_opts).await?;
        let sqlite = shared_deps.sqlite;
        let provider = shared_deps.provider;

        let mev_filter = TxsFilter::new(&self.filter, None, self.conn_opts.trace.as_ref(), false)?;

        let ens_lookup = if ENSLookup::sync_lookup(mev_filter.ens_query()).await {
            ENSLookup::Sync
        } else {
            ENSLookup::Async(shared_deps.ens_lookup_worker)
        };

        let latest_block = provider.get_block_number().await?;
        let block_range = BlocksRange::from_str(&self.blocks, latest_block)?;

        if !mev_filter.top_metadata {
            println!("{}", SEPARATORER);
        }
        for block_number in block_range.from..=block_range.to {
            process_block(
                &provider,
                &sqlite,
                block_number,
                &ens_lookup,
                &shared_deps.symbols_lookup_worker,
                &mev_filter,
                &self.conn_opts,
            )
            .await?;
        }

        if mev_filter.top_metadata {
            println!("{}", SEPARATORER);
        }
        // Allow async ENS and symbols lookups to finish
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}
