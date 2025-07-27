use eyre::Result;
use mevlog::misc::rpc_urls::get_chain_info;

#[derive(Debug, clap::Parser)]
pub struct RpcUrlsArgs {
    #[arg(help = "Chain ID to get RPC URLs for")]
    pub chain_id: u64,
    #[arg(
        help = "Timeout in seconds for a healthy RPC URL",
        default_value = "1",
        long,
        short = 't'
    )]
    pub rpc_timeout_sec: u64,
}

impl RpcUrlsArgs {
    pub async fn run(&self) -> Result<()> {
        let chain = get_chain_info(self.chain_id, self.rpc_timeout_sec).await?;

        println!("Chain: {} ({})", chain.name, chain.chain);
        println!("Chain ID: {}", chain.chain_id);

        if chain.benchmarked_rpc_urls.is_empty() {
            println!("No healthy RPC URLs available");
        } else {
            println!(
                "\nRPC URLs (responding under {}ms):",
                self.rpc_timeout_sec * 1000
            );
            for (i, (url, response_time)) in chain.benchmarked_rpc_urls.iter().enumerate() {
                println!("  {}. {} ({}ms)", i + 1, url, response_time);
            }
        }

        Ok(())
    }
}
