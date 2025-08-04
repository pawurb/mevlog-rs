use alloy::providers::Provider;
use clap::Parser;
use eyre::Result;
use mevlog::{
    misc::{
        ens_utils::ENSLookup,
        shared_init::{init_deps, ConnOpts, SharedOpts},
        utils::get_native_token_price,
    },
    models::{
        mev_block::generate_block,
        txs_filter::{TxsFilter, TxsFilterOpts},
    },
};

#[derive(Debug, Parser)]
pub struct WatchArgs {
    #[command(flatten)]
    filter_opts: TxsFilterOpts,

    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl WatchArgs {
    pub async fn run(&self) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        let txs_filter = TxsFilter::new(&self.filter_opts, None, &self.shared_opts, true)?;

        let ens_lookup =
            ENSLookup::lookup_mode(txs_filter.ens_query(), deps.ens_lookup_worker, &deps.chain)
                .await;

        let native_token_price = get_native_token_price(&deps.chain, &deps.provider).await?;

        let mut current_block_number = deps.provider.get_block_number().await? - 1;

        loop {
            let new_block_number = deps.provider.get_block_number().await?;
            if new_block_number == current_block_number {
                // TODO config sleep delay
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
            current_block_number = new_block_number;
            let mev_block = generate_block(
                &deps.provider,
                &deps.sqlite,
                current_block_number,
                &ens_lookup,
                &deps.symbols_lookup_worker,
                &txs_filter,
                &self.shared_opts,
                &deps.chain,
                &deps.rpc_url,
                native_token_price,
            )
            .await?;

            mev_block.print_with_format(&self.shared_opts.format);
        }

        #[allow(unreachable_code)]
        Ok(())
    }
}
