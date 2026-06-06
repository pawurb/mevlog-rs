use alloy::{network::ReceiptResponse, primitives::TxHash, providers::Provider};
use eyre::{Result, bail};
use mevlog::{
    db::txs::{
        display_sql::{logs_display_query, tx_display_query},
        indexing::index_block_range,
        raw_query::run_raw_query,
    },
    misc::{
        shared_init::{ConnOpts, OutputFormat, TraceMode, init_deps},
        sql_macros::{NATIVE_TOKEN_PRICE_MACRO, substitute_sql_macros},
        tx_tracing::backfill_coinbase_transfers,
        utils::get_native_token_price,
    },
};
use serde_json::Value;

#[derive(Debug, clap::Parser)]
pub struct TxArgs {
    #[arg(help = "Transaction hash to display")]
    pub tx_hash: TxHash,

    #[arg(long, help = "Embed the transaction's logs in the output")]
    pub logs: bool,

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
        let tx_index = receipt
            .transaction_index()
            .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", self.tx_hash))?;

        index_block_range(block_number, block_number, 1, &deps).await?;

        if let Some(mode) = &self.evm_trace {
            backfill_coinbase_transfers(
                block_number,
                block_number,
                mode,
                &deps.provider,
                &deps.chain,
                &deps.rpc_url,
                &deps.txs,
            )
            .await?;
        }

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

        let mut tx = run_raw_query(&sql, &deps.txs_read_path)?
            .rows
            .into_iter()
            .next()
            .ok_or_else(|| eyre::eyre!("Transaction {} not found in local store", self.tx_hash))?;

        if self.logs {
            let logs_sql = logs_display_query(&format!(
                "block_number = {block_number} AND tx_index = {tx_index}"
            ));
            let logs: Vec<Value> = run_raw_query(&logs_sql, &deps.txs_read_path)?
                .rows
                .into_iter()
                .map(fold_topics)
                .collect();
            if let Value::Object(map) = &mut tx {
                map.insert("logs".to_string(), Value::Array(logs));
            }
        }

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&tx)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&tx)?),
            OutputFormat::Table | OutputFormat::Csv => {
                bail!("tx supports only json and json-pretty output formats")
            }
        }

        Ok(())
    }
}

/// Folds a log row's `topic0..topic3` columns into a single `topics` array,
/// keeping the contiguous non-null topics (`topic0` is the event signature
/// hash). The per-topic columns are dropped from the row.
fn fold_topics(row: Value) -> Value {
    let Value::Object(mut map) = row else {
        return row;
    };

    let mut topics = Vec::new();
    let mut ended = false;
    for key in ["topic0", "topic1", "topic2", "topic3"] {
        match map.remove(key) {
            Some(Value::String(topic)) if !ended => topics.push(Value::String(topic)),
            _ => ended = true,
        }
    }
    map.insert("topics".to_string(), Value::Array(topics));

    Value::Object(map)
}
