use alloy::{network::ReceiptResponse, primitives::TxHash, providers::Provider};
use eyre::{Result, bail};
use mevlog::{
    db::txs::{
        indexing::index_block_range,
        models::{log::Log, transaction::Transaction},
    },
    misc::{
        shared_init::{ConnOpts, OutputFormat, TraceMode, init_deps},
        tx_tracing::backfill_coinbase_transfers,
        utils::get_native_token_price,
    },
    models::json::{log_json::LogJson, transaction_json::TransactionJson},
};

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

        let where_sql = format!("tx_hash = X'{}'", hex::encode(self.tx_hash));
        let tx = Transaction::query_where(&where_sql, &deps.txs)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| eyre::eyre!("Transaction {} not found in local store", self.tx_hash))?;

        let mut tx_json = TransactionJson::from_record(&tx, native_token_price);

        if self.logs {
            let where_sql = format!("block_number = {block_number} AND tx_index = {tx_index}");
            let logs = Log::query_where(&where_sql, &deps.txs).await?;
            tx_json.logs = logs.iter().map(LogJson::from_record).collect();
        }

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&tx_json)?),
            OutputFormat::JsonPretty => {
                println!("{}", serde_json::to_string_pretty(&tx_json)?)
            }
            OutputFormat::Table | OutputFormat::Csv => {
                bail!("tx supports only json and json-pretty output formats")
            }
        }

        Ok(())
    }
}
