use std::time::Instant;

use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{indexing::index_block_range, raw_query::run_raw_query},
    misc::{
        args_parsing::BlocksRange,
        shared_init::{ConnOpts, OutputFormat, SharedOpts, init_deps},
        sql_macros::substitute_sql_macros,
        tx_tracing::backfill_coinbase_transfers,
        utils::get_native_token_price,
    },
    models::json::query_response::{
        QueryParams, rows_to_csv, rows_to_table, serialize_query_response,
    },
};

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
        help = "Read-only SQL to run against the local txs DB \
                (tables: transactions, logs, blocks). Blob columns (addresses, \
                hashes) are output as 0x-hex; addresses/hashes in predicates must \
                be given as blob literals, e.g. WHERE from_address = X'1111...1111'. \
                Macros must be wrapped in braces. {LATEST_BLOCK()} expands to the chain's \
                current latest block number (fetched via RPC), e.g. WHERE block_number > \
                {LATEST_BLOCK()} - 100. {NATIVE_TOKEN_PRICE()} expands to the native token's \
                USD price (from --native-token-price or a Chainlink oracle). \
                {RESOLVE_ENS(\"name.eth\")} expands to the resolved address as a blob literal \
                (Ethereum mainnet only), e.g. WHERE from_address = {RESOLVE_ENS(\"vitalik.eth\")}"
    )]
    sql: String,
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
        let (cached_blocks, new_blocks) =
            index_block_range(block_range.from, block_range.to, self.batch_size, &deps).await?;

        // Backfill direct coinbase payments for any untraced txs in range. Runs
        // over the local store, so it also covers blocks indexed earlier without
        // --evm-trace.
        if let Some(mode) = &self.shared_opts.evm_trace {
            backfill_coinbase_transfers(
                block_range.from,
                block_range.to,
                mode,
                &deps.provider,
                &deps.chain,
                &deps.rpc_url,
                &deps.txs,
            )
            .await?;
        }

        let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        chain_info.native_token_price = native_token_price;
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        let sql = substitute_sql_macros(
            &self.sql,
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
