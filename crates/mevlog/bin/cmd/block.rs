use std::time::Instant;

use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::block_display_query, indexing::index_block_range, raw_query::run_raw_query,
    },
    misc::{
        args_parsing::BlocksRange,
        shared_init::{ConnOpts, OutputFormat, init_deps},
    },
    models::json::query_response::{
        QueryParams, rows_to_csv, rows_to_table, serialize_query_response,
    },
};

#[derive(Debug, clap::Parser)]
pub struct BlockArgs {
    #[arg(help = "Block number or 'latest'")]
    pub block: String,

    #[arg(long, help = "Get N-offset latest block")]
    pub latest_offset: Option<u64>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl BlockArgs {
    /// Convenience wrapper around `query`: indexes a single block and emits its
    /// metadata through the same
    /// [`QueryResponse`](mevlog::models::json::query_response::QueryResponse)
    /// envelope, with the one matching block in `result`. Use `block-txs` for the
    /// block's transactions.
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        let range = BlocksRange::from_str(&self.block, &deps.provider, self.latest_offset).await?;
        if range.from != range.to {
            bail!("block expects a single block number or 'latest', not a range");
        }
        let block_number = range.from;

        let start_time = Instant::now();

        let (cached_blocks, new_blocks) =
            index_block_range(block_number, block_number, 1, &deps).await?;

        let chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        let sql = block_display_query(&format!("block_number = {block_number}"));
        let result = run_raw_query(&sql, &deps.txs_read_path)?;
        if result.rows.is_empty() {
            bail!("Block {block_number} not found in local store");
        }

        match format {
            OutputFormat::Csv => print!("{}", rows_to_csv(&result.columns, &result.rows)?),
            OutputFormat::Table => print!("{}", rows_to_table(&result.columns, &result.rows)),
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let pretty = matches!(format, OutputFormat::JsonPretty);
                let query = QueryParams {
                    blocks: block_number.to_string(),
                    sql: Some(sql),
                    evm_trace: None,
                    evm_calls: false,
                    evm_ops: false,
                    evm_state_diff: false,
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
