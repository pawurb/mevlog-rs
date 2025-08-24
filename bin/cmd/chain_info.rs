use eyre::Result;
use mevlog::{
    misc::{
        rpc_urls::{get_chain_info, get_chain_info_no_benchmark},
        shared_init::OutputFormat,
    },
    ChainInfoJson, ChainInfoNoRpcsJson, RpcUrlInfo,
};

#[derive(Debug, clap::Parser)]
pub struct ChainInfoArgs {
    #[arg(
        long,
        help = "Skip RPC URL benchmarking and only show chain information"
    )]
    pub skip_urls: bool,

    #[arg(long, help = "Chain ID to get information for")]
    pub chain_id: u64,

    #[arg(long, help = "RPC timeout in milliseconds", default_value = "1000")]
    pub rpc_timeout_ms: u64,

    #[arg(long, help = "Number of RPC URLs to return", default_value = "5")]
    pub rpcs_limit: usize,
}

impl ChainInfoArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let chain_info_raw = if self.skip_urls {
            get_chain_info_no_benchmark(self.chain_id).await?
        } else {
            let info = get_chain_info(self.chain_id, self.rpc_timeout_ms, self.rpcs_limit).await?;
            if info.benchmarked_rpc_urls.is_empty() {
                return Err(eyre::eyre!(
                    "No working RPC URLs found for chain ID {}",
                    self.chain_id
                ));
            }
            info
        };

        if self.skip_urls {
            let no_rpcs = ChainInfoNoRpcsJson {
                chain_id: self.chain_id,
                name: chain_info_raw.name.clone(),
                currency: chain_info_raw.native_currency.symbol.clone(),
                explorer_url: chain_info_raw.explorers.first().map(|e| e.url.clone()),
            };
            self.output_no_rpcs(no_rpcs, format).await?;
        } else {
            let rpc_urls = chain_info_raw
                .benchmarked_rpc_urls
                .iter()
                .map(|(url, response_time)| RpcUrlInfo {
                    url: url.clone(),
                    response_time_ms: *response_time,
                })
                .collect();

            let response = ChainInfoJson {
                chain_id: self.chain_id,
                name: chain_info_raw.name.clone(),
                currency: chain_info_raw.native_currency.symbol.clone(),
                explorer_url: chain_info_raw.explorers.first().map(|e| e.url.clone()),
                rpc_timeout_ms: self.rpc_timeout_ms,
                rpc_urls,
            };
            self.output_with_rpcs(response, format).await?;
        }

        Ok(())
    }

    async fn output_no_rpcs(&self, info: ChainInfoNoRpcsJson, format: OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Text => {
                println!("Chain Information");
                println!("================");
                println!("Chain ID: {}", info.chain_id);
                println!("Name: {}", info.name);
                println!("Currency: {}", info.currency);

                if let Some(explorer_url) = &info.explorer_url {
                    println!("Explorer URL: {explorer_url}");
                } else {
                    println!("Explorer URL: N/A");
                }
            }
            OutputFormat::Json | OutputFormat::JsonStream => {
                println!("{}", serde_json::to_string(&info)?);
            }
            OutputFormat::JsonPretty | OutputFormat::JsonPrettyStream => {
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
        }
        Ok(())
    }

    async fn output_with_rpcs(&self, info: ChainInfoJson, format: OutputFormat) -> Result<()> {
        match format {
            OutputFormat::Text => {
                println!("Chain Information");
                println!("================");
                println!("Chain ID: {}", info.chain_id);
                println!("Name: {}", info.name);
                println!("Currency: {}", info.currency);

                if let Some(explorer_url) = &info.explorer_url {
                    println!("Explorer URL: {explorer_url}");
                } else {
                    println!("Explorer URL: N/A");
                }

                if info.rpc_urls.is_empty() {
                    println!("No healthy RPC URLs available");
                } else {
                    println!("\nRPC URLs (responding under {}ms):", info.rpc_timeout_ms);
                    for (i, rpc_info) in info.rpc_urls.iter().enumerate() {
                        println!(
                            "  {}. {} ({}ms)",
                            i + 1,
                            rpc_info.url,
                            rpc_info.response_time_ms
                        );
                    }
                }
            }
            OutputFormat::Json | OutputFormat::JsonStream => {
                println!("{}", serde_json::to_string(&info)?);
            }
            OutputFormat::JsonPretty | OutputFormat::JsonPrettyStream => {
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
        }
        Ok(())
    }
}
