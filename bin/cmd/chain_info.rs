use eyre::Result;
use mevlog::misc::rpc_urls::{get_chain_info, get_chain_info_no_benchmark};
use serde_json::json;

#[derive(Debug, clap::Parser)]
pub struct ChainInfoArgs {
    #[arg(
        long,
        help = "Output format ('text' or 'json')",
        default_value = "text"
    )]
    pub format: ChainInfoFormat,

    #[arg(
        long,
        help = "Skip RPC URL benchmarking and only show chain information"
    )]
    pub skip_urls: bool,

    #[arg(long, help = "Chain ID to get information for")]
    pub chain_id: u64,

    #[arg(long, help = "RPC timeout in milliseconds", default_value = "1000")]
    pub rpc_timeout_ms: u64,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ChainInfoFormat {
    Text,
    Json,
    JsonPretty,
}

impl ChainInfoArgs {
    pub async fn run(&self) -> Result<()> {
        let chain_info = if self.skip_urls {
            get_chain_info_no_benchmark(self.chain_id).await?
        } else {
            let info = get_chain_info(self.chain_id, self.rpc_timeout_ms).await?;
            if info.benchmarked_rpc_urls.is_empty() {
                return Err(eyre::eyre!(
                    "No working RPC URLs found for chain ID {}",
                    self.chain_id
                ));
            }
            info
        };

        match self.format {
            ChainInfoFormat::Text => {
                println!("Chain Information");
                println!("================");
                println!("Chain ID: {}", self.chain_id);
                println!("Name: {}", chain_info.name);
                println!("Currency: {}", chain_info.native_currency.symbol);

                if let Some(explorer) = chain_info.explorers.first() {
                    println!("Explorer URL: {}", explorer.url);
                } else {
                    println!("Explorer URL: N/A");
                }

                if !self.skip_urls {
                    if chain_info.benchmarked_rpc_urls.is_empty() {
                        println!("No healthy RPC URLs available");
                    } else {
                        println!("\nRPC URLs (responding under {}ms):", self.rpc_timeout_ms);
                        for (i, (url, response_time)) in
                            chain_info.benchmarked_rpc_urls.iter().enumerate()
                        {
                            println!("  {}. {} ({}ms)", i + 1, url, response_time);
                        }
                    }
                }
            }
            ChainInfoFormat::Json | ChainInfoFormat::JsonPretty => {
                let mut info = json!({
                    "chain_id": self.chain_id,
                    "name": chain_info.name,
                    "currency": chain_info.native_currency.symbol,
                    "explorer_url": chain_info.explorers.first().map(|e| e.url.clone()),
                });

                if !self.skip_urls {
                    let rpc_urls: Vec<serde_json::Value> = chain_info
                        .benchmarked_rpc_urls
                        .iter()
                        .map(|(url, response_time)| {
                            json!({
                                "url": url,
                                "response_time_ms": response_time
                            })
                        })
                        .collect();

                    info["rpc_timeout_ms"] = json!(self.rpc_timeout_ms);
                    info["rpc_urls"] = json!(rpc_urls);
                }

                match self.format {
                    ChainInfoFormat::Json => {
                        println!("{}", serde_json::to_string(&info)?);
                    }
                    ChainInfoFormat::JsonPretty => {
                        println!("{}", serde_json::to_string_pretty(&info)?);
                    }
                    ChainInfoFormat::Text => unreachable!(),
                }
            }
        }

        Ok(())
    }
}
