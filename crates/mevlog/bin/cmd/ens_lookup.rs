use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat},
};
use revm::primitives::Address;

#[derive(Debug, clap::Parser)]
pub struct EnsLookupArgs {
    #[arg(help = "Address to reverse-resolve to an ENS name")]
    pub address: Address,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl EnsLookupArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        let output = cmds::ens_lookup::ens_lookup(self.address, &self.conn_opts).await?;

        match format {
            OutputFormat::Json => println!("{}", serde_json::to_string(&output)?),
            OutputFormat::JsonPretty => println!("{}", serde_json::to_string_pretty(&output)?),
            OutputFormat::Csv | OutputFormat::Table | OutputFormat::Html => {
                eyre::bail!(
                    "'csv', 'table' and 'html' formats are only supported by the query command"
                )
            }
        }

        Ok(())
    }
}
