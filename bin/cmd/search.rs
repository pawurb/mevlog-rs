use alloy::providers::Provider;
use eyre::Result;
use mevlog::{
    misc::{
        args_parsing::BlocksRange,
        ens_utils::ENSLookup,
        shared_init::{init_deps, ConnOpts, SharedOpts},
        utils::{get_native_token_price, SEPARATORER},
    },
    models::{
        mev_block::generate_block,
        txs_filter::{TxsFilter, TxsFilterOpts},
    },
};

#[derive(Debug, clap::Parser)]
pub struct SearchArgs {
    #[arg(short = 'b', long, help_heading = "Block number or range to filter by (e.g., '22030899', 'latest', '22030800:22030900' '50:latest', '50:'", num_args(1..))]
    blocks: String,

    #[command(flatten)]
    filter_opts: TxsFilterOpts,

    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl SearchArgs {
    pub async fn run(&self) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        let txs_filter = TxsFilter::new(&self.filter_opts, None, &self.shared_opts, false)?;

        let ens_lookup =
            ENSLookup::lookup_mode(txs_filter.ens_query(), deps.ens_lookup_worker, &deps.chain)
                .await;

        let native_token_price = get_native_token_price(&deps.chain, &deps.provider).await?;

        let latest_block = deps.provider.get_block_number().await?;
        let block_range = BlocksRange::from_str(&self.blocks, latest_block)?;

        if !txs_filter.top_metadata {
            println!("{SEPARATORER}");
        }
        for block_number in block_range.from..=block_range.to {
            let mev_block = generate_block(
                &deps.provider,
                &deps.sqlite,
                block_number,
                &ens_lookup,
                &deps.symbols_lookup_worker,
                &txs_filter,
                &self.shared_opts,
                &deps.chain,
                &deps.rpc_url,
                native_token_price,
            )
            .await?;

            mev_block.print();
        }

        if txs_filter.top_metadata {
            println!("{SEPARATORER}");
        }
        // Allow async ENS and symbols lookups to finish
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}
