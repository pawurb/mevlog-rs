use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat},
};

#[derive(Debug, clap::Parser)]
pub struct EnsResolveArgs {
    #[arg(help = "ENS name to resolve to an address (e.g. 'vitalik.eth')")]
    pub name: String,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl EnsResolveArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let output = cmds::ens_resolve::ens_resolve(&self.name, &self.conn_opts).await?;

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
