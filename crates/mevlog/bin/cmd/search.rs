use std::time::Instant;

use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    misc::{
        args_parsing::BlocksRange,
        data_fetch::fetch_blocks_batch,
        ens_utils::ENSLookup,
        shared_init::{ConnOpts, OutputFormat, SharedOpts, init_deps},
        symbol_utils::ERC20SymbolsLookup,
        utils::get_native_token_price,
    },
    models::{
        json::mev_transaction_json::{SearchQueryParams, serialize_json_response},
        mev_block::{PreFetchedBlockData, generate_block},
    },
};

#[derive(Debug, clap::Parser)]
pub struct SearchArgs {
    #[arg(short = 'b', long, help_heading = "Block number or range to collect (e.g., '22030899', 'latest', '22030800:22030900' '50:latest', '50:'", num_args(1..))]
    blocks: String,

    #[command(flatten)]
    shared_opts: SharedOpts,

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
}

impl SearchArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        if self.shared_opts.evm_trace.is_none() {
            if self.shared_opts.evm_calls {
                bail!("'--evm-calls' is supported only with --evm-trace [rpc|revm] enabled")
            }
            if self.shared_opts.evm_ops {
                bail!("'--evm-ops' is supported only with --evm-trace [rpc|revm] enabled")
            }
            if self.shared_opts.evm_state_diff {
                bail!("'--evm-state-diff' is supported only with --evm-trace [rpc|revm] enabled")
            }
        }

        let ens_lookup = ENSLookup::lookup_mode(
            None,
            deps.ens_lookup_worker,
            &deps.chain,
            self.shared_opts.ens,
            &deps.provider,
        )
        .await?;

        let symbols_lookup = ERC20SymbolsLookup::lookup_mode(
            deps.symbols_lookup_worker,
            self.shared_opts.erc20_symbols,
        );

        let native_token_price = get_native_token_price(
            &deps.chain,
            &deps.provider,
            self.shared_opts.native_token_price,
        )
        .await?;

        let block_range =
            BlocksRange::from_str(&self.blocks, &deps.provider, self.latest_offset).await?;

        if let Some(max_range) = self.max_range {
            let range_size = block_range.size();
            if range_size > max_range {
                bail!(
                    "Block range size {} exceeds maximum allowed range of {}",
                    range_size,
                    max_range
                );
            }
        }

        let start_time = Instant::now();
        let mut mev_blocks = vec![];
        let blocks: Vec<u64> = (block_range.from..=block_range.to).rev().collect();

        for chunk in blocks.chunks(self.batch_size) {
            let start_block = *chunk.iter().min().unwrap();
            let end_block = *chunk.iter().max().unwrap();

            let batch_data = fetch_blocks_batch(
                start_block,
                end_block,
                &deps.chain,
                &deps.sqlite,
                &symbols_lookup,
            )
            .await?;

            for &block_number in chunk {
                let pre_fetched = PreFetchedBlockData {
                    txs_data: batch_data
                        .txs_by_block
                        .get(&block_number)
                        .cloned()
                        .unwrap_or_default(),
                    logs_data: batch_data
                        .logs_by_block
                        .get(&block_number)
                        .cloned()
                        .unwrap_or_default(),
                };

                let json_opts = self.shared_opts.json_serialize_opts();
                let mev_block = generate_block(
                    &deps.provider,
                    &deps.sqlite,
                    block_number,
                    &ens_lookup,
                    None,
                    false,
                    &self.shared_opts,
                    &deps.chain,
                    &deps.rpc_url,
                    native_token_price,
                    json_opts.include_logs,
                    pre_fetched,
                )
                .await?;

                mev_blocks.push(mev_block);
            }
        }

        let json_opts = self.shared_opts.json_serialize_opts();
        let transactions_json: Vec<_> = mev_blocks
            .iter()
            .flat_map(|block| block.transactions_json())
            .collect();

        let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        chain_info.native_token_price = native_token_price;
        let duration_ns = start_time.elapsed().as_nanos() as u64;
        let query = SearchQueryParams {
            command: "search",
            blocks: self.blocks.clone(),
            evm_trace: self.shared_opts.evm_trace.clone(),
            evm_calls: self.shared_opts.evm_calls,
            evm_ops: self.shared_opts.evm_ops,
            evm_state_diff: self.shared_opts.evm_state_diff,
        };

        let pretty = matches!(format, OutputFormat::JsonPretty);
        println!(
            "{}",
            serialize_json_response(
                &transactions_json,
                json_opts,
                pretty,
                &chain_info,
                duration_ns,
                query,
            )
            .unwrap()
        );

        // Allow async ENS and erc20 symbols lookups to catch up
        if self.shared_opts.erc20_symbols || self.shared_opts.ens {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Ok(())
    }
}
