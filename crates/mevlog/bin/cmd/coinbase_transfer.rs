use alloy::primitives::TxHash;
use eyre::{Result, bail};
use mevlog::{
    db::sigs::models::chain::Chain,
    misc::{
        shared_init::{ConnOpts, OutputFormat, TraceMode, resolve_conn},
        tx_tracing::coinbase_transfer_for_tx,
    },
    models::evm_chain::EVMChain,
};

#[derive(Debug, clap::Parser)]
pub struct CoinbaseTransferArgs {
    #[arg(help = "Transaction hash to compute the direct coinbase payment for")]
    pub tx_hash: TxHash,

    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub evm_trace: Option<TraceMode>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl CoinbaseTransferArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let Some(mode) = &self.evm_trace else {
            bail!("--evm-trace [rpc|revm] must be specified")
        };

        let conn = resolve_conn(&self.conn_opts).await?;
        let chain = EVMChain::new(Chain::unknown(conn.chain_id as i64), conn.rpc_url.clone())?;

        let transfer =
            coinbase_transfer_for_tx(self.tx_hash, mode, &conn.provider, &chain, &conn.rpc_url)
                .await?;

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&transfer)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&transfer)?),
            OutputFormat::Table => println!("{transfer}"),
            OutputFormat::Csv => {
                eyre::bail!("'csv' format is not supported by the coinbase-transfer command")
            }
        }

        Ok(())
    }
}
