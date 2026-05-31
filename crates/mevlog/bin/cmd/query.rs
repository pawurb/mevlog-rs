use std::time::Instant;

use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::models::{
        log::Log,
        transaction::{Transaction, TransactionJson},
    },
    misc::{
        args_parsing::BlocksRange,
        data_fetch::fetch_blocks_batch,
        shared_init::{ConnOpts, OutputFormat, SharedOpts, init_deps},
        symbol_utils::ERC20SymbolsLookup,
        utils::get_native_token_price,
    },
    models::{
        json::mev_transaction_json::{QueryParams, serialize_query_response},
        mev_block::TxData,
    },
};
use revm::primitives::{FixedBytes, TxKind, U256};
use tracing::info;

#[derive(Debug, clap::Parser)]
pub struct QueryArgs {
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

impl QueryArgs {
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

        // Only fetch blocks that are not already in the local store. Indexed
        // blocks (including empty ones, tracked in `indexed_blocks`) are skipped.
        let missing =
            Transaction::missing_blocks(block_range.from, block_range.to, &deps.txs).await?;

        let new_blocks = missing.len() as u64;
        let cached_blocks = block_range.size().saturating_sub(new_blocks);

        info!(
            "Blocks: {} cached, {} to fetch ({} total)",
            cached_blocks,
            new_blocks,
            block_range.size()
        );

        let ranges = contiguous_ranges(&missing);
        let total_batches: usize = ranges
            .iter()
            .map(|(s, e)| ((e - s + 1) as usize).div_ceil(self.batch_size))
            .sum();
        let mut batch_idx = 0;

        for (run_start, run_end) in ranges {
            let run_blocks: Vec<u64> = (run_start..=run_end).collect();

            for chunk in run_blocks.chunks(self.batch_size) {
                let start_block = *chunk.first().unwrap();
                let end_block = *chunk.last().unwrap();

                batch_idx += 1;
                info!(
                    "Fetching blocks {}-{} (batch {}/{})",
                    start_block, end_block, batch_idx, total_batches
                );

                let batch_data = fetch_blocks_batch(
                    start_block,
                    end_block,
                    &deps.chain,
                    &deps.sqlite,
                    &symbols_lookup,
                )
                .await?;

                let mut chunk_txs: Vec<Transaction> = vec![];
                for &block_number in chunk {
                    let Some(txs_data) = batch_data.txs_by_block.get(&block_number) else {
                        continue;
                    };

                    for (tx_index, tx_data) in txs_data.iter().enumerate() {
                        chunk_txs.push(build_transaction(block_number, tx_index as u64, tx_data));
                    }
                }

                let mut chunk_logs: Vec<Log> = vec![];
                for &block_number in chunk {
                    if let Some(logs) = batch_data.logs_by_block.get(&block_number) {
                        chunk_logs.extend(logs.iter().map(|l| Log::from_mev_log(block_number, l)));
                    }
                }

                // Persist logs before txs: `save_batch` marks the chunk's blocks
                // as indexed, so a block is only flagged once its logs have landed.
                // `chunk` (not just blocks with txs) is passed so empty blocks are
                // still recorded in `indexed_blocks`.
                Log::save_batch(&chunk_logs, &deps.txs).await?;
                Transaction::save_batch(&chunk_txs, chunk, &deps.txs).await?;
            }
        }

        // Read the full requested range back from the local store so that
        // previously indexed blocks are included alongside the freshly fetched ones.
        let where_sql = format!(
            "block_number BETWEEN {} AND {}",
            block_range.from, block_range.to
        );
        let txs = Transaction::query_where(&where_sql, &deps.txs).await?;

        let transactions_json: Vec<TransactionJson> =
            txs.iter().map(TransactionJson::from).collect();

        let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        chain_info.native_token_price = native_token_price;
        let duration_ns = start_time.elapsed().as_nanos() as u64;
        let query = QueryParams {
            command: "query",
            blocks: self.blocks.clone(),
            evm_trace: self.shared_opts.evm_trace.clone(),
            evm_calls: self.shared_opts.evm_calls,
            evm_ops: self.shared_opts.evm_ops,
            evm_state_diff: self.shared_opts.evm_state_diff,
        };

        let pretty = matches!(format, OutputFormat::JsonPretty);
        println!(
            "{}",
            serialize_query_response(
                &transactions_json,
                pretty,
                &chain_info,
                duration_ns,
                cached_blocks,
                new_blocks,
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

/// Collapses a sorted, deduplicated list of block numbers into contiguous
/// `(start, end)` inclusive ranges so each gap is fetched as a single batch.
fn contiguous_ranges(blocks: &[u64]) -> Vec<(u64, u64)> {
    let mut ranges: Vec<(u64, u64)> = vec![];

    for &block in blocks {
        match ranges.last_mut() {
            Some(last) if block == last.1 + 1 => last.1 = block,
            _ => ranges.push((block, block)),
        }
    }

    ranges
}

/// Builds a barebones [`Transaction`] record from fetched RPC data.
///
/// No SQLite insert and no signature resolution yet — the method signature is
/// left unset and only the 4-byte selector is captured from the calldata.
fn build_transaction(block_number: u64, tx_index: u64, tx_data: &TxData) -> Transaction {
    let req = &tx_data.req;

    let to_address = match req.to {
        Some(TxKind::Call(address)) => Some(address),
        // `TxKind::Create` or an unset target → contract creation.
        _ => None,
    };

    let signature_hash = req
        .input
        .input
        .as_ref()
        .filter(|input| input.len() >= 4)
        .map(|input| FixedBytes::<4>::from_slice(&input[..4]));

    Transaction {
        block_number,
        tx_index,
        tx_hash: tx_data.tx_hash,
        nonce: req.nonce.unwrap_or(0),
        from_address: req.from.expect("tx `from` address missing"),
        to_address,
        value: req.value.unwrap_or(U256::ZERO),
        gas_limit: req.gas.unwrap_or(0),
        gas_used: tx_data.receipt.gas_used,
        effective_gas_price: tx_data.receipt.effective_gas_price,
        gas_price: req.gas_price.unwrap_or(0),
        max_fee_per_gas: req.max_fee_per_gas.unwrap_or(0),
        max_priority_fee_per_gas: req.max_priority_fee_per_gas.unwrap_or(0),
        transaction_type: req.transaction_type,
        success: tx_data.receipt.success,
        signature_hash,
        signature: None,
    }
}
