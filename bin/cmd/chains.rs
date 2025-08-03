use eyre::Result;
use mevlog::misc::rpc_urls::get_all_chains;

#[derive(Debug, clap::Parser)]
pub struct ChainsArgs {
    #[arg(
        help = "Filter chains by name (case-insensitive substring match)",
        long,
        short = 'f'
    )]
    pub filter: Option<String>,
}

impl ChainsArgs {
    pub async fn run(&self) -> Result<()> {
        let chains = get_all_chains().await?;

        let mut filtered_chains = if let Some(filter) = &self.filter {
            let filter_lower = filter.to_lowercase();
            chains
                .into_iter()
                .filter(|chain| {
                    chain.name.to_lowercase().contains(&filter_lower)
                        || chain.chain.to_lowercase().contains(&filter_lower)
                })
                .collect()
        } else {
            chains
        };

        filtered_chains.sort_by_key(|chain| chain.chain_id);

        if filtered_chains.is_empty() {
            if let Some(filter) = &self.filter {
                println!("No chains found matching filter: {filter}");
            } else {
                println!("No chains available");
            }
            return Ok(());
        }

        println!("Available chains ({} total):", filtered_chains.len());
        println!("{:<4} {:<12} Name", "#", "Chain ID");
        println!("{}", "-".repeat(60));

        for (index, chain) in filtered_chains.iter().enumerate() {
            println!("{} - {:<12} {}", index + 1, chain.chain_id, chain.name);
        }

        Ok(())
    }
}
