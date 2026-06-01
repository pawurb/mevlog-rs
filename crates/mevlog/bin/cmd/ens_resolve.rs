use eyre::{Result, bail};
use mevlog::misc::{
    ens_utils::{ens_addr_lookup, ensure_ens_supported},
    shared_init::{ConnOpts, OutputFormat, resolve_conn},
};
use serde::Serialize;

#[derive(Debug, clap::Parser)]
pub struct EnsResolveArgs {
    #[arg(help = "ENS name to resolve to an address (e.g. 'vitalik.eth')")]
    pub name: String,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

#[derive(Serialize)]
struct EnsResolveJson {
    name: String,
    address: String,
}

impl EnsResolveArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let resolved = resolve_conn(&self.conn_opts).await?;
        ensure_ens_supported(resolved.chain_id)?;

        let Some(address) = ens_addr_lookup(&self.name, &resolved.provider).await? else {
            bail!("{} is not a registered ENS name", self.name);
        };

        let output = EnsResolveJson {
            name: self.name.clone(),
            address: format!("{address:#x}"),
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
