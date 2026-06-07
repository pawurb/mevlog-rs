use std::time::Instant;

use alloy::{network::ReceiptResponse, primitives::TxHash, providers::Provider};
use eyre::Result;
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::logs_display_query, indexing::index_block_range, raw_query::run_raw_query,
    },
    misc::shared_init::{ConnOpts, OutputFormat, init_deps},
    models::json::query_response::{
        QueryParams, rows_to_csv, rows_to_table, serialize_query_response,
    },
};

#[derive(Debug, clap::Parser)]
pub struct TxLogsArgs {
    #[arg(help = "Transaction hash whose logs to display")]
    pub tx_hash: TxHash,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl TxLogsArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        let receipt = deps
            .provider
            .get_transaction_receipt(self.tx_hash)
            .await?
            .ok_or_else(|| eyre::eyre!("Transaction {} not found", self.tx_hash))?;
        let block_number = receipt
            .block_number()
            .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", self.tx_hash))?;
        let tx_index = receipt
            .transaction_index()
            .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", self.tx_hash))?;

        let start_time = Instant::now();

        let (cached_blocks, new_blocks) =
            index_block_range(block_number, block_number, 1, &deps).await?;

        let chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        // The logs SELECT has no macros (no USD columns), so it runs as-is.
        let sql = logs_display_query(&format!(
            "block_number = {block_number} AND tx_index = {tx_index}"
        ));
        let result = run_raw_query(&sql, &deps.txs_read_path)?;

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
