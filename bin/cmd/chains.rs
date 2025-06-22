use eyre::Result;
use mevlog::models::evm_chain::EVMChainType;

#[derive(Debug, clap::Parser)]
pub struct ChainsArgs {}

impl ChainsArgs {
    pub async fn run(&self) -> Result<()> {
        let supported_chains_text = EVMChainType::supported_chains_text();
        println!("{supported_chains_text}");
        Ok(())
    }
}
