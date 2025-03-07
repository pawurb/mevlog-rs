use alloy::providers::Provider;
use clap::Parser;
use eyre::Result;
use futures_util::StreamExt;
use mevlog::misc::shared_init::{init_deps, ConnOpts, ProviderType};
use mevlog::misc::utils::SEPARATORER;
use mevlog::models::mev_block::process_block;
use mevlog::models::txs_filter::{SharedFilterOpts, TxsFilter};

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
        let ens_lookup = shared_deps.ens_lookup;
        let provider = shared_deps.provider;

        println!("{}", SEPARATORER);
        let mev_filter = TxsFilter::new(&self.filter, None, self.conn_opts.trace.as_ref(), true)?;
        let block_number = provider.get_block_number().await?;
        process_block(
            &provider,
            sqlite.clone(),
            block_number,
            &ens_lookup,
            &mev_filter,
            &self.conn_opts,
        )
        .await?;

        match shared_deps.provider_type {
            ProviderType::WS => {
                let mut blocks_stream = provider.subscribe_blocks().await?.into_stream();

                while let Some(block) = blocks_stream.next().await {
                    process_block(
                        &provider,
                        sqlite.clone(),
                        block.number,
                        &ens_lookup,
                        &mev_filter,
                        &self.conn_opts,
                    )
                    .await?;
                }
            }
            ProviderType::HTTP => {
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
                        sqlite.clone(),
                        current_block_number,
                        &ens_lookup,
                        &mev_filter,
                        &self.conn_opts,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }
}
