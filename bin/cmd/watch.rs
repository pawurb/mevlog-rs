use alloy::providers::Provider;
use clap::Parser;
use eyre::Result;
use mevlog::{
    misc::{
        ens_utils::ENSLookup,
        shared_init::{init_deps, ConnOpts},
        utils::SEPARATORER,
    },
    models::{
        mev_block::process_block,
        txs_filter::{SharedFilterOpts, TxsFilter},
    },
};

#[derive(Debug, Parser)]
pub struct WatchArgs {
    #[command(flatten)]
    filter: SharedFilterOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl WatchArgs {
    pub async fn run(&self) -> Result<()> {
        let shared_deps = init_deps(&self.conn_opts).await?;
        let sqlite = shared_deps.sqlite;
        let provider = shared_deps.provider;

        println!("{SEPARATORER}");
        let mev_filter = TxsFilter::new(&self.filter, None, self.conn_opts.trace.as_ref(), true)?;

        let ens_lookup = ENSLookup::lookup_mode(
            mev_filter.ens_query(),
            shared_deps.ens_lookup_worker,
            &shared_deps.chain,
        )
        .await;

        let block_number = provider.get_block_number().await?;
        process_block(
            &provider,
            &sqlite,
            block_number,
            &ens_lookup,
            &shared_deps.symbols_lookup_worker,
            &mev_filter,
            &self.conn_opts,
            &shared_deps.chain,
        )
        .await?;

        let mut current_block_number = provider.get_block_number().await?;

        loop {
            let new_block_number = provider.get_block_number().await?;
            if new_block_number == current_block_number {
                // TODO config sleep delay
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
            current_block_number = new_block_number;
            process_block(
                &provider,
                &sqlite,
                current_block_number,
                &ens_lookup,
                &shared_deps.symbols_lookup_worker,
                &mev_filter,
                &self.conn_opts,
                &shared_deps.chain,
            )
            .await?;
        }

        #[allow(unreachable_code)]
        Ok(())
    }
}
