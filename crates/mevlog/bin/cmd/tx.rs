use std::time::Instant;

use alloy::{network::ReceiptResponse, primitives::TxHash, providers::Provider};
use eyre::{Result, bail};
use mevlog::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::tx_display_query, indexing::index_block_range,
        models::transaction::Transaction, raw_query::run_raw_query,
    },
    misc::{
        shared_init::{ConnOpts, OutputFormat, TraceMode, init_deps},
        sql_macros::{NATIVE_TOKEN_PRICE_MACRO, substitute_sql_macros},
        tx_tracing::coinbase_transfer_for_tx,
        utils::get_native_token_price,
    },
    models::json::query_response::{
        QueryParams, rows_to_csv, rows_to_table, serialize_query_response,
    },
};

#[derive(Debug, clap::Parser)]
pub struct TxArgs {
    #[arg(help = "Transaction hash to display")]
    pub tx_hash: TxHash,

    #[arg(
        long,
        help = "EVM tracing mode ('revm' or 'rpc'); enables coinbase/full cost"
    )]
    pub evm_trace: Option<TraceMode>,

    #[arg(long, help = "Native token price in USD (overrides the chain oracle)")]
    pub native_token_price: Option<f64>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl TxArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let deps = init_deps(&self.conn_opts).await?;

        let native_token_price =
            get_native_token_price(&deps.chain, &deps.provider, self.native_token_price).await?;

        let receipt = deps
            .provider
            .get_transaction_receipt(self.tx_hash)
            .await?
            .ok_or_else(|| eyre::eyre!("Transaction {} not found", self.tx_hash))?;
        let block_number = receipt
            .block_number()
            .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", self.tx_hash))?;

        let start_time = Instant::now();

        let (cached_blocks, new_blocks) =
            index_block_range(block_number, block_number, 1, &deps).await?;

        if let Some(mode) = &self.evm_trace {
            let coinbase_transfer = coinbase_transfer_for_tx(
                self.tx_hash,
                mode,
                &deps.provider,
                &deps.chain,
                &deps.rpc_url,
            )
            .await?;
            Transaction::update_coinbase_transfer(
                self.tx_hash,
                coinbase_transfer.amount_wei,
                &deps.txs,
            )
            .await?;
        }

        let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
        chain_info.native_token_price = native_token_price;
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        let sql = tx_display_query(&format!("tx_hash = X'{}'", hex::encode(self.tx_hash)));
        // Without a price the macro can't resolve, so substitute NULL; the USD
        // columns then render as null rather than erroring.
        let sql = if native_token_price.is_some() {
            substitute_sql_macros(
                &sql,
                &deps.provider,
                deps.chain.chain_id,
                native_token_price,
            )
            .await?
        } else {
            sql.replace(NATIVE_TOKEN_PRICE_MACRO, "NULL")
        };

        let result = run_raw_query(&sql, &deps.txs_read_path)?;
        if result.rows.is_empty() {
            bail!("Transaction {} not found in local store", self.tx_hash);
        }

        match format {
            OutputFormat::Csv => print!("{}", rows_to_csv(&result.columns, &result.rows)?),
            OutputFormat::Table => print!("{}", rows_to_table(&result.columns, &result.rows)),
            OutputFormat::Json | OutputFormat::JsonPretty => {
                let pretty = matches!(format, OutputFormat::JsonPretty);
                let query = QueryParams {
                    blocks: block_number.to_string(),
                    sql: Some(sql),
                    evm_trace: self.evm_trace.clone(),
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
