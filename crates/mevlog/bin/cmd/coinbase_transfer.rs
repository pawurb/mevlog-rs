use alloy::primitives::TxHash;
use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat, TraceMode},
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
        let transfer = cmds::coinbase_transfer::coinbase_transfer(
            self.tx_hash,
            self.evm_trace.as_ref(),
            &self.conn_opts,
        )
        .await?;

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&transfer)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&transfer)?),
            OutputFormat::Table => println!("{transfer}"),
            OutputFormat::Csv => {
                eyre::bail!("'csv' format is not supported by the evm-coinbase-transfer command")
            }
        }

        Ok(())
    }
}
