use alloy::primitives::TxHash;
use eyre::{Result, bail};
use mevlog::misc::{
    shared_init::{ConnOpts, OutputFormat, TraceMode, init_deps},
    tx_tracing::calls_for_tx,
};

#[derive(Debug, clap::Parser)]
pub struct EvmTracesArgs {
    #[arg(help = "Transaction hash to extract call traces for")]
    pub tx_hash: TxHash,

    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub evm_trace: Option<TraceMode>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl EvmTracesArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let Some(mode) = &self.evm_trace else {
            bail!("--evm-trace [rpc|revm] must be specified")
        };

        let deps = init_deps(&self.conn_opts).await?;

        let calls = calls_for_tx(
            self.tx_hash,
            mode,
            &deps.provider,
            deps.chain.as_ref(),
            &deps.rpc_url,
            &deps.sqlite,
        )
        .await?;

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&calls)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&calls)?),
            OutputFormat::Table => {
                for call in &calls {
                    println!("{} -> {} {}", call.from, call.to, call.signature);
                }
            }
            OutputFormat::Csv => {
                let mut writer = csv::Writer::from_writer(vec![]);
                writer.write_record(["from", "to", "signature", "signature_hash"])?;
                for call in &calls {
                    writer.write_record([
                        call.from.to_string().as_str(),
                        call.to.to_string().as_str(),
                        call.signature.as_str(),
                        call.signature_hash.as_deref().unwrap_or(""),
                    ])?;
                }
                let bytes = writer.into_inner().map_err(|e| eyre::eyre!(e))?;
                print!("{}", String::from_utf8(bytes)?);
            }
        }

        Ok(())
    }
}
