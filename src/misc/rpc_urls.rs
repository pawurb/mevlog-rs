use std::time::{Duration, Instant};

use alloy::providers::Provider;
use eyre::{bail, Result};
use serde::Deserialize;
use tokio::time::sleep;

use crate::misc::shared_init::init_provider;

#[derive(Debug, Deserialize, Clone)]
pub struct RpcEndpoint {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChainInfo {
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    pub name: String,
    pub chain: String,
    #[serde(rename = "rpc")]
    pub rpc_endpoints: Vec<RpcEndpoint>,
    #[serde(skip)]
    pub benchmarked_rpc_urls: Vec<(String, u64)>,
}

pub async fn get_chain_info(chain_id: u64, timeout_sec: u64) -> Result<ChainInfo> {
    let client = reqwest::Client::new();

    let response = client.get("https://chainlist.org/rpcs.json").send().await?;

    let chains: Vec<ChainInfo> = response.json().await?;

    let mut chain = chains
        .into_iter()
        .find(|c| c.chain_id == chain_id)
        .ok_or_else(|| eyre::eyre!("Chain ID {} not found", chain_id))?;

    // Benchmark RPC URLs
    let benchmark_futures = chain
        .rpc_endpoints
        .iter()
        .filter(|endpoint| endpoint.url.starts_with("https://"))
        .filter(|endpoint| !endpoint.url.contains("${"))
        .map(|endpoint| async move {
            match benchmark_url(endpoint.url.clone(), timeout_sec).await {
                Ok(duration) => Some((endpoint.url.clone(), duration)),
                Err(_) => None,
            }
        })
        .collect::<Vec<_>>();

    let mut benchmarked_rpc_urls: Vec<(String, u64)> =
        futures_util::future::join_all(benchmark_futures)
            .await
            .into_iter()
            .flatten()
            .collect();

    // Sort by duration (fastest first)
    benchmarked_rpc_urls.sort_by_key(|(_, duration)| *duration);

    chain.benchmarked_rpc_urls = benchmarked_rpc_urls;

    Ok(chain)
}

pub async fn benchmark_url(url: String, timeout_sec: u64) -> Result<u64> {
    let provider = init_provider(&url).await?;
    let start = Instant::now();
    tokio::select! {
        block_number = provider.get_block_number() => {
            if block_number.is_err() {
                bail!("RPC URL returned an error");
            } else {
                Ok(start.elapsed().as_millis() as u64)
            }
        }
        _ = sleep(Duration::from_secs(timeout_sec)) => {
            bail!("RPC URL timed out");
        }
    }
}
