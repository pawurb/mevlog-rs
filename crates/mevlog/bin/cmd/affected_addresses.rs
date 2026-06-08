use alloy::primitives::TxHash;
use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat, TraceMode},
};

#[derive(Debug, clap::Parser)]
pub struct AffectedAddressesArgs {
    #[arg(help = "Transaction hash to inspect for affected addresses")]
    pub tx_hash: TxHash,

    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub evm_trace: Option<TraceMode>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl AffectedAddressesArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let addresses = cmds::affected_addresses::affected_addresses(
            self.tx_hash,
            self.evm_trace.as_ref(),
            &self.conn_opts,
        )
        .await?;

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&addresses)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&addresses)?),
            OutputFormat::Table => {
                for address in addresses {
                    println!("{address}");
                }
            }
            OutputFormat::Csv => {
                println!("address");
                for address in addresses {
                    println!("{address}");
                }
            }
        }

        Ok(())
    }
}
