use eyre::Result;
use mevlog::{
    cmds::{self, chain_info::ChainInfoOutput},
    misc::shared_init::OutputFormat,
};
use serde::Serialize;

#[derive(Debug, clap::Parser)]
pub struct ChainInfoArgs {
    #[arg(
        long,
        help = "Skip RPC URL benchmarking and only show chain information"
    )]
    pub skip_rpcs: bool,

    #[arg(long, help = "Chain ID to get information for")]
    pub chain_id: Option<u64>,

    #[arg(long, help = "RPC URL to derive chain ID from")]
    pub rpc_url: Option<String>,

    #[arg(long, help = "RPC timeout in milliseconds", default_value = "1000")]
    pub rpc_timeout_ms: u64,

    #[arg(long, help = "Number of RPC URLs to return", default_value = "5")]
    pub rpcs_limit: usize,
}

impl ChainInfoArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        let output = cmds::chain_info::chain_info(
            self.chain_id,
            self.rpc_url.as_deref(),
            self.skip_rpcs,
            self.rpc_timeout_ms,
            self.rpcs_limit,
        )
        .await?;

        match output {
            ChainInfoOutput::Full(info) => print_json(&info, format),
            ChainInfoOutput::NoRpcs(info) => print_json(&info, format),
        }
    }
}

fn print_json<T: Serialize>(info: &T, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(info)?);
        }
        OutputFormat::JsonPretty => {
            println!("{}", serde_json::to_string_pretty(info)?);
        }
        OutputFormat::Csv | OutputFormat::Table => {
            eyre::bail!("'csv' and 'table' formats are only supported by the query command")
        }
    }
    Ok(())
}
