use eyre::Result;
use mevlog::{cmds, misc::shared_init::OutputFormat};

#[derive(Debug, clap::Parser)]
pub struct ChainsArgs {
    #[arg(
        help = "Filter chains by name (case-insensitive substring match)",
        long,
        short = 'f'
    )]
    pub filter: Option<String>,
    #[arg(help = "Limit the number of chains returned", long, short = 'l')]
    pub limit: Option<usize>,
    #[arg(
        help = "Filter by specific chain IDs (can be used multiple times)",
        long,
        action = clap::ArgAction::Append
    )]
    pub chain_id: Vec<u64>,
}

impl ChainsArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        let chains_entries =
            cmds::chains::chains(self.filter.as_deref(), self.limit, &self.chain_id).await?;

        match format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(&chains_entries)?);
            }
            OutputFormat::JsonPretty => {
                println!("{}", serde_json::to_string_pretty(&chains_entries)?);
            }
            OutputFormat::Csv | OutputFormat::Table | OutputFormat::Html => {
                eyre::bail!(
                    "'csv', 'table' and 'html' formats are only supported by the query command"
                )
            }
        }

        Ok(())
    }
}
