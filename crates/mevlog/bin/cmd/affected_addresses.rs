use alloy::primitives::TxHash;
use eyre::{Result, bail};
use mevlog::{
    db::sigs::models::chain::Chain,
    misc::{
        shared_init::{ConnOpts, OutputFormat, TraceMode, resolve_conn},
        tx_tracing::affected_addresses_for_tx,
    },
    models::evm_chain::EVMChain,
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
        let Some(mode) = &self.evm_trace else {
            bail!("--evm-trace [rpc|revm] must be specified")
        };

        let conn = resolve_conn(&self.conn_opts).await?;
        let chain = EVMChain::new(Chain::unknown(conn.chain_id as i64), conn.rpc_url.clone())?;

        let addresses =
            affected_addresses_for_tx(self.tx_hash, mode, &conn.provider, &chain, &conn.rpc_url)
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
