use eyre::{Result, bail};
use mevlog::{
    misc::{
        args_parsing::BlocksRange,
        ens_utils::ENSLookup,
        shared_init::{ConnOpts, OutputFormat, SharedOpts, init_deps},
        symbol_utils::ERC20SymbolsLookup,
        utils::get_native_token_price,
    },
    models::{
        json::mev_transaction_json::MEVTransactionJson,
        mev_block::{PreFetchedBlockData, fetch_blocks_batch, generate_block},
        txs_filter::{TxsFilter, TxsFilterOpts},
    },
};
use revm::primitives::{Address, U256};

#[derive(Debug, Clone, PartialEq)]
pub enum SortField {
    GasPrice,
    GasUsed,
    FullTxCost,
    TxCost,
    Erc20Transfer(Address), // Token contract address
}

impl std::str::FromStr for SortField {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gas-price" => Ok(SortField::GasPrice),
            "gas-used" => Ok(SortField::GasUsed),
            "full-tx-cost" => Ok(SortField::FullTxCost),
            "tx-cost" => Ok(SortField::TxCost),
            _ => {
                if s.starts_with("erc20Transfer|") {
                    let token_address = s
                        .strip_prefix("erc20Transfer|")
                        .ok_or("Token address required after 'erc20Transfer|'")?;

                    match token_address.parse::<Address>() {
                        Ok(address) => Ok(SortField::Erc20Transfer(address)),
                        Err(_) => Err(
                            "Invalid token address format. Expected valid Ethereum address"
                                .to_string(),
                        ),
                    }
                } else {
                    Err(format!(
                        "Invalid sort field: '{}'. Expected one of: gas-price, gas-used, tx-cost, full-tx-cost, or erc20Transfer|<token_address>",
                        s
                    ))
                }
            }
        }
    }
}

fn extract_erc20_transfer_amount(
    transaction: &MEVTransactionJson,
    token_address: &Address,
) -> U256 {
    transaction
        .log_groups
        .iter()
        .filter(|group| group.source == *token_address)
        .flat_map(|group| &group.logs)
        .filter(|log| log.signature == "Transfer(address,address,uint256)")
        .filter_map(|log| log.amount.as_ref().and_then(|amt| amt.parse::<U256>().ok()))
        .sum()
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
        help = "Sort transactions by field (gas-price, gas-used, tx-cost, full-tx-cost, erc20Transfer|<token_address>)"
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

        if (self.limit.is_some() || self.sort.is_some()) && !format.non_stream_json() {
            {
                bail!(
                    "--limit and --sort are not available in --format {:?}",
                    format
                );
            }
        }

        if let Some(sort) = &self.sort
            && sort == &SortField::FullTxCost
            && self.shared_opts.trace.is_none()
        {
            bail!("--sort full-tx-cost is only available with --trace enabled")
        }

        let erc20_sort_token = match &self.sort {
            Some(SortField::Erc20Transfer(token_address)) => Some(*token_address),
            _ => None,
        };

        let txs_filter = TxsFilter::new(
            &self.filter_opts,
            None,
            &self.shared_opts,
            false,
            erc20_sort_token,
        )?;

        let ens_query = txs_filter
            .from_ens_query()
            .or_else(|| txs_filter.to_ens_query());

        let ens_lookup = ENSLookup::lookup_mode(
            ens_query,
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
                txs_filter.show_erc20_transfer_amount,
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

                let mev_block = generate_block(
                    &deps.provider,
                    &deps.sqlite,
                    block_number,
                    &ens_lookup,
                    &txs_filter,
                    &self.shared_opts,
                    &deps.chain,
                    &deps.rpc_url,
                    native_token_price,
                    pre_fetched,
                )
                .await?;

                if format.is_stream() {
                    mev_block.print_with_format(&format);
                } else {
                    mev_blocks.push(mev_block);
                }
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
        SortField::Erc20Transfer(token_address) => {
            transactions_json.sort_by(|a, b| {
                let a_amount = extract_erc20_transfer_amount(a, token_address);
                let b_amount = extract_erc20_transfer_amount(b, token_address);
                match sort_dir {
                    SortDirection::Desc => b_amount
                        .cmp(&a_amount)
                        .then_with(|| a.tx_hash.cmp(&b.tx_hash)),
                    SortDirection::Asc => a_amount
                        .cmp(&b_amount)
                        .then_with(|| a.tx_hash.cmp(&b.tx_hash)),
                }
            });
        }
    }
}
