use eyre::Result;
use mevlog::misc::{
    rpc_urls::{get_chain_info, get_chain_info_no_benchmark},
    shared_init::{init_deps, ConnOpts},
    utils::get_native_token_price,
};
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

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ChainInfoFormat {
    Text,
    Json,
    JsonPretty,
}

impl ChainInfoArgs {
    pub async fn run(&self) -> Result<()> {
        let chain_id = match self.conn_opts.chain_id {
            Some(chain_id) => chain_id,
            None => {
                eyre::bail!("--chain-id must be provided");
            }
        };

        if self.conn_opts.rpc_url.is_none() && self.skip_urls {
            eyre::bail!("for --skip-urls, --rpc-url must be provided");
        }

        let chain_info = if self.skip_urls {
            get_chain_info_no_benchmark(chain_id).await?
        } else {
            let info = get_chain_info(chain_id, self.conn_opts.rpc_timeout_ms).await?;
            if info.benchmarked_rpc_urls.is_empty() {
                return Err(eyre::eyre!(
                    "No working RPC URLs found for chain ID {}",
                    chain_id
                ));
            }
            info
        };

        let deps = init_deps(&self.conn_opts).await?;
        let token_price = get_native_token_price(&deps.chain, &deps.provider).await?;

        match self.format {
            ChainInfoFormat::Text => {
                println!("Chain Information");
                println!("================");
                println!("Chain ID: {}", deps.chain.chain_id);
                println!("Name: {}", deps.chain.name);
                println!("Currency: {}", deps.chain.currency_symbol);
                println!(
                    "Explorer URL: {}",
                    deps.chain.explorer_url.as_deref().unwrap_or("N/A")
                );
                if let Some(price) = token_price {
                    println!("Current Token Price: ${price:.2}");
                } else {
                    println!("Current Token Price: N/A");
                }

                if !self.skip_urls {
                    if chain_info.benchmarked_rpc_urls.is_empty() {
                        println!("No healthy RPC URLs available");
                    } else {
                        println!(
                            "\nRPC URLs (responding under {}ms):",
                            self.conn_opts.rpc_timeout_ms
                        );
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
                    "chain_id": deps.chain.chain_id,
                    "name": deps.chain.name,
                    "currency": deps.chain.currency_symbol,
                    "explorer_url": deps.chain.explorer_url,
                    "current_token_price": token_price
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

                    info["rpc_timeout_ms"] = json!(self.conn_opts.rpc_timeout_ms);
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
