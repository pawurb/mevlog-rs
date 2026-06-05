use std::time::Instant;

use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{
        models::{block::Block, log::Log, transaction::Transaction},
        raw_query::run_raw_query,
    },
    misc::{
        args_parsing::BlocksRange,
        data_fetch::fetch_blocks_batch,
        shared_init::{ConnOpts, OutputFormat, SharedOpts, init_deps},
        sql_macros::substitute_sql_macros,
        utils::get_native_token_price,
    },
    models::json::query_response::{
        QueryParams, rows_to_csv, rows_to_table, serialize_query_response,
    },
};
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

    #[arg(
        long,
        help = "Custom read-only SQL to run against the local txs DB \
                (tables: transactions, logs, blocks). When omitted, all \
                txs in the block range are returned. Blob columns (addresses, \
                hashes) are output as 0x-hex; addresses/hashes in predicates must \
                be given as blob literals, e.g. WHERE from_address = X'1111...1111'. \
                Macros must be wrapped in braces. {LATEST_BLOCK()} expands to the chain's \
                current latest block number (fetched via RPC), e.g. WHERE block_number > \
                {LATEST_BLOCK()} - 100. {NATIVE_TOKEN_PRICE()} expands to the native token's \
                USD price (from --native-token-price or a Chainlink oracle). \
                {RESOLVE_ENS(\"name.eth\")} expands to the resolved address as a blob literal \
                (Ethereum mainnet only), e.g. WHERE from_address = {RESOLVE_ENS(\"vitalik.eth\")}"
    )]
    sql: Option<String>,
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
        // blocks (including empty ones, tracked by the `blocks` table) are skipped.
        let missing = Block::missing_blocks(block_range.from, block_range.to, &deps.txs).await?;

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

                let batch_data =
                    fetch_blocks_batch(start_block, end_block, &deps.chain, &deps.sqlite).await?;

                let mut chunk_txs: Vec<Transaction> = vec![];
                for &block_number in chunk {
                    if let Some(txs) = batch_data.txs_by_block.get(&block_number) {
                        chunk_txs.extend(txs.iter().cloned());
                    }
                }

                let mut chunk_logs: Vec<Log> = vec![];
                for &block_number in chunk {
                    if let Some(logs) = batch_data.logs_by_block.get(&block_number) {
                        chunk_logs.extend(logs.iter().map(|l| Log::from_mev_log(block_number, l)));
                    }
                }

                let mut chunk_blocks: Vec<Block> = vec![];
                for &block_number in chunk {
                    if let Some(block) = batch_data.blocks_by_block.get(&block_number) {
                        chunk_blocks.push(block.clone());
                    }
                }

                // Persist blocks last: inserting a `blocks` row marks the block as
                // indexed, so a block is only flagged once its txs and logs have
                // landed. Every block in `chunk` (including empty ones) yields a
                // block row, so empty blocks are still recorded as indexed.
                Log::save_batch(&chunk_logs, &deps.txs).await?;
                Transaction::save_batch(&chunk_txs, &deps.txs).await?;
                Block::save_batch(&chunk_blocks, &deps.txs).await?;
            }
        }

        let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        chain_info.native_token_price = native_token_price;
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        // Default to reading the full requested range back from the local store so
        // previously indexed blocks are included alongside freshly fetched ones.
        let sql = self.sql.clone().unwrap_or_else(|| {
            format!(
                "SELECT * FROM transactions \
                 WHERE block_number BETWEEN {} AND {} \
                 ORDER BY block_number DESC, tx_index ASC",
                block_range.from, block_range.to
            )
        });
        let sql = substitute_sql_macros(
            &sql,
            &deps.provider,
            deps.chain.chain_id,
            native_token_price,
        )
        .await?;
        let result = run_raw_query(&sql, &deps.txs_read_path)?;

        match format {
            OutputFormat::Csv | OutputFormat::Table => {
                // Tabular formats emit only the result rows, no envelope metadata.
                let output = if matches!(format, OutputFormat::Csv) {
                    rows_to_csv(&result.columns, &result.rows)?
                } else {
                    rows_to_table(&result.columns, &result.rows)
                };
                print!("{output}");
            }
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let pretty = matches!(format, OutputFormat::JsonPretty);
                let query = QueryParams {
                    blocks: self.blocks.clone(),
                    sql: Some(sql),
                    evm_trace: self.shared_opts.evm_trace.clone(),
                    evm_calls: self.shared_opts.evm_calls,
                    evm_ops: self.shared_opts.evm_ops,
                    evm_state_diff: self.shared_opts.evm_state_diff,
                };

                let output = serialize_query_response(
                    result.rows,
                    pretty,
                    chain_info,
                    duration_ns,
                    cached_blocks,
                    new_blocks,
                    query,
                )?;

                println!("{output}");
            }
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
