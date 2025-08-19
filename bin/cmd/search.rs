use alloy::providers::Provider;
use eyre::{bail, Result};
use mevlog::{
    misc::{
        args_parsing::BlocksRange,
        ens_utils::ENSLookup,
        shared_init::{init_deps, ConnOpts, OutputFormat, SharedOpts},
        symbol_utils::ERC20SymbolsLookup,
        utils::get_native_token_price,
    },
    models::{
        json::mev_transaction_json::MEVTransactionJson,
        mev_block::generate_block,
        txs_filter::{TxsFilter, TxsFilterOpts},
    },
};

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum SortField {
    GasPrice,
    GasUsed,
    FullTxCost,
    TxCost,
}

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, clap::Parser)]
pub struct SearchArgs {
    #[arg(short = 'b', long, help_heading = "Block number or range to filter by (e.g., '22030899', 'latest', '22030800:22030900' '50:latest', '50:'", num_args(1..))]
    blocks: String,

    #[arg(long, help = "Limit the number of transactions returned")]
    limit: Option<usize>,

    #[arg(
        long,
        help = "Sort transactions by field (gas-price, gas-used, tx-cost, full-tx-cost)"
    )]
    sort: Option<SortField>,

    #[arg(long, help = "Sort direction (desc, asc)", default_value = "desc")]
    sort_dir: SortDirection,

    #[command(flatten)]
    filter_opts: TxsFilterOpts,

    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl SearchArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        if (self.limit.is_some() || self.sort.is_some()) && !format.non_stream_json() {
            {
                bail!(
                    "--limit and --sort are not available in --format {:?}",
                    format
                );
            }
        }

        if let Some(sort) = &self.sort {
            if sort == &SortField::FullTxCost && self.shared_opts.trace.is_none() {
                bail!("--sort full-tx-cost is only available with --trace enabled")
            }
        }

        let txs_filter = TxsFilter::new(&self.filter_opts, None, &self.shared_opts, false)?;

        let ens_lookup = ENSLookup::lookup_mode(
            txs_filter.ens_query(),
            deps.ens_lookup_worker,
            &deps.chain,
            self.shared_opts.ens,
        )
        .await?;

        let symbols_lookup = ERC20SymbolsLookup::lookup_mode(
            deps.symbols_lookup_worker,
            self.shared_opts.erc20_symbols,
        );

        let (native_token_price, latest_block) =
            match tokio::try_join!(get_native_token_price(&deps.chain, &deps.provider), async {
                deps.provider
                    .get_block_number()
                    .await
                    .map_err(eyre::Report::from)
            }) {
                Ok((native_token_price, latest_block)) => (native_token_price, latest_block),
                Err(e) => bail!("Error getting native token price or latest block: {:?}", e),
            };

        let block_range = BlocksRange::from_str(&self.blocks, latest_block)?;

        let mut mev_blocks = vec![];

        for block_number in (block_range.from..=block_range.to).rev() {
            let mev_block = generate_block(
                &deps.provider,
                &deps.sqlite,
                block_number,
                &ens_lookup,
                &symbols_lookup,
                &txs_filter,
                &self.shared_opts,
                &deps.chain,
                &deps.rpc_url,
                native_token_price,
            )
            .await?;

            if format.is_stream() {
                mev_block.print_with_format(&format);
            } else {
                mev_blocks.push(mev_block);
            }
        }

        if !format.is_stream() {
            let mut transactions_json: Vec<_> = mev_blocks
                .iter()
                .flat_map(|block| block.transactions_json())
                .collect();

            if let Some(sort_field) = &self.sort {
                sort_transactions(&mut transactions_json, sort_field, &self.sort_dir);
            }

            if let Some(limit) = self.limit {
                transactions_json.truncate(limit);
            }

            match format {
                OutputFormat::Json => {
                    println!("{}", serde_json::to_string(&transactions_json).unwrap());
                }
                OutputFormat::JsonPretty => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&transactions_json).unwrap()
                    );
                }
                _ => {
                    unreachable!()
                }
            }
        }

        // Allow async ENS and erc20 symbols lookups to catch up
        if self.shared_opts.erc20_symbols || self.shared_opts.ens {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Ok(())
    }
}

fn sort_transactions(
    transactions_json: &mut [MEVTransactionJson],
    sort_field: &SortField,
    sort_dir: &SortDirection,
) {
    match sort_field {
        SortField::GasPrice => match sort_dir {
            SortDirection::Desc => transactions_json.sort_by(|a, b| {
                b.gas_price
                    .cmp(&a.gas_price)
                    .then_with(|| a.tx_hash.cmp(&b.tx_hash))
            }),
            SortDirection::Asc => transactions_json.sort_by(|a, b| {
                a.gas_price
                    .cmp(&b.gas_price)
                    .then_with(|| a.tx_hash.cmp(&b.tx_hash))
            }),
        },
        SortField::GasUsed => match sort_dir {
            SortDirection::Desc => transactions_json.sort_by(|a, b| {
                b.gas_used
                    .cmp(&a.gas_used)
                    .then_with(|| a.tx_hash.cmp(&b.tx_hash))
            }),
            SortDirection::Asc => transactions_json.sort_by(|a, b| {
                a.gas_used
                    .cmp(&b.gas_used)
                    .then_with(|| a.tx_hash.cmp(&b.tx_hash))
            }),
        },
        SortField::TxCost => {
            transactions_json.sort_by(|a, b| {
                let a_tx_cost = a.gas_used as u128 * a.gas_price;
                let b_tx_cost = b.gas_used as u128 * b.gas_price;
                match sort_dir {
                    SortDirection::Desc => b_tx_cost
                        .cmp(&a_tx_cost)
                        .then_with(|| a.tx_hash.cmp(&b.tx_hash)),
                    SortDirection::Asc => a_tx_cost
                        .cmp(&b_tx_cost)
                        .then_with(|| a.tx_hash.cmp(&b.tx_hash)),
                }
            });
        }
        SortField::FullTxCost => {
            transactions_json.sort_by(|a, b| {
                let a_cost = a.full_tx_cost.expect("must be traced");
                let b_cost = b.full_tx_cost.expect("must be traced");
                match sort_dir {
                    SortDirection::Desc => {
                        b_cost.cmp(&a_cost).then_with(|| a.tx_hash.cmp(&b.tx_hash))
                    }
                    SortDirection::Asc => {
                        a_cost.cmp(&b_cost).then_with(|| a.tx_hash.cmp(&b.tx_hash))
                    }
                }
            });
        }
    }
}
