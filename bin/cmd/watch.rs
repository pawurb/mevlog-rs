use alloy::providers::Provider;
use clap::Parser;
use eyre::Result;
use mevlog::{
    misc::{
        ens_utils::ENSLookup,
        shared_init::{init_deps, ConnOpts, SharedOpts},
        utils::{get_native_token_price, SEPARATORER},
    },
    models::{
        mev_block::generate_block,
        txs_filter::{TxsFilter, TxsFilterOpts},
    },
};

#[derive(Debug, Parser)]
pub struct WatchArgs {
    #[command(flatten)]
    filter: TxsFilterOpts,

    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl WatchArgs {
    pub async fn run(&self) -> Result<()> {
        let shared_deps = init_deps(&self.conn_opts).await?;
        let sqlite = shared_deps.sqlite;
        let provider = shared_deps.provider;

        println!("{SEPARATORER}");
        let mev_filter = TxsFilter::new(&self.filter, None, &self.shared_opts, true)?;

        let ens_lookup = ENSLookup::lookup_mode(
            mev_filter.ens_query(),
            shared_deps.ens_lookup_worker,
            &shared_deps.chain,
        )
        .await;

        let native_token_price = get_native_token_price(&shared_deps.chain, &provider).await?;

        let mut current_block_number = provider.get_block_number().await? - 1;

        loop {
            let new_block_number = provider.get_block_number().await?;
            if new_block_number == current_block_number {
                // TODO config sleep delay
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
            current_block_number = new_block_number;
            let mev_block = generate_block(
                &provider,
                &sqlite,
                current_block_number,
                &ens_lookup,
                &shared_deps.symbols_lookup_worker,
                &mev_filter,
                &self.shared_opts,
                &shared_deps.chain,
                &shared_deps.rpc_url,
                native_token_price,
            )
            .await?;

            mev_block.print();
        }

        #[allow(unreachable_code)]
        Ok(())
    }
}
