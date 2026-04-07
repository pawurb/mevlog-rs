use eyre::Result;
use mevlog::{
    ChainEntryJson,
    misc::{rpc_urls::get_all_chains, shared_init::OutputFormat},
};

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
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let chains = get_all_chains().await?;

        let mut filtered_chains = chains;

        if let Some(filter) = &self.filter {
            let filter_lower = filter.to_lowercase();
            filtered_chains.retain(|chain| {
                chain.name.to_lowercase().contains(&filter_lower)
                    || chain.chain.to_lowercase().contains(&filter_lower)
            });
        }

        if !self.chain_id.is_empty() {
            filtered_chains.retain(|chain| self.chain_id.contains(&chain.chain_id));
        }

        filtered_chains.sort_by_key(|chain| chain.chain_id);

        if let Some(limit) = self.limit {
            filtered_chains.truncate(limit);
        }

        let chains_entries: Vec<ChainEntryJson> = filtered_chains
            .iter()
            .map(|chain| ChainEntryJson {
                chain_id: chain.chain_id,
                name: chain.name.clone(),
                chain: chain.chain.clone(),
                explorer_url: chain.explorers.first().map(|e| e.url.clone()),
            })
            .collect();

        match format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string(&chains_entries)?);
            }
            OutputFormat::JsonPretty => {
                println!("{}", serde_json::to_string_pretty(&chains_entries)?);
            }
        }

        Ok(())
    }
}
