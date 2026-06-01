use eyre::{Result, bail};
use mevlog::misc::{
    ens_utils::{ens_name_lookup, ensure_ens_supported},
    shared_init::{ConnOpts, OutputFormat, resolve_conn},
};
use revm::primitives::Address;
use serde::Serialize;

#[derive(Debug, clap::Parser)]
pub struct EnsLookupArgs {
    #[arg(help = "Address to reverse-resolve to an ENS name")]
    pub address: Address,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

#[derive(Serialize)]
struct EnsLookupJson {
    address: String,
    name: String,
}

impl EnsLookupArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let resolved = resolve_conn(&self.conn_opts).await?;
        ensure_ens_supported(resolved.chain_id)?;

        let Some(name) = ens_name_lookup(self.address, &resolved.provider).await? else {
            bail!("No ENS name set for {:#x}", self.address);
        };

        let output = EnsLookupJson {
            address: format!("{:#x}", self.address),
            name,
        };

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&output)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&output)?),
            OutputFormat::Csv | OutputFormat::Table => {
                eyre::bail!("'csv' and 'table' formats are only supported by the query command")
            }
        }

        Ok(())
    }
}
