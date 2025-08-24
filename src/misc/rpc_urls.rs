use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use alloy::providers::Provider;
use eyre::{bail, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::misc::shared_init::init_provider;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RpcEndpoint {
    pub url: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NativeCurrency {
    pub name: String,
    pub symbol: String,
    pub decimals: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Explorer {
    pub url: String,
}

const CHAINLIST_URL: &str = "https://chainlist.org/rpcs.json";
const CACHE_KEY: &str = "chainlist_rpcs";
const CACHE_EXPIRY_SECONDS: u64 = 60;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChainInfo {
    #[serde(rename = "chainId")]
    pub chain_id: u64,
    pub name: String,
    pub chain: String,
    #[serde(rename = "rpc")]
    pub rpc_endpoints: Vec<RpcEndpoint>,
    #[serde(rename = "nativeCurrency")]
    pub native_currency: NativeCurrency,
    #[serde(default)]
    pub explorers: Vec<Explorer>,
    #[serde(skip)]
    pub benchmarked_rpc_urls: Vec<(String, u64)>,
}

pub async fn get_chain_info_no_benchmark(chain_id: u64) -> Result<ChainInfo> {
    let chains = get_all_chains().await?;

    let chain = chains
        .into_iter()
        .find(|c| c.chain_id == chain_id)
        .ok_or_else(|| eyre::eyre!("Chain ID {} not found", chain_id))?;

    Ok(chain)
}

pub async fn get_chain_info(chain_id: u64, timeout_ms: u64, limit: usize) -> Result<ChainInfo> {
    let chains = get_all_chains().await?;

    let mut chain = chains
        .into_iter()
        .find(|c| c.chain_id == chain_id)
        .ok_or_else(|| eyre::eyre!("Chain ID {} not found", chain_id))?;

    let benchmark_futures = chain
        .rpc_endpoints
        .iter()
        .filter(|endpoint| endpoint.url.starts_with("https://"))
        .filter(|endpoint| !endpoint.url.contains("${"))
        .map(|endpoint| async move {
            match benchmark_url(endpoint.url.clone(), timeout_ms).await {
                Ok(duration) => Some((endpoint.url.clone(), duration)),
                Err(_) => None,
            }
        })
        .collect::<Vec<_>>();

    let mut benchmarked_rpc_urls: Vec<(String, u64)> =
        futures_util::stream::iter(benchmark_futures)
            .buffer_unordered(10)
            .filter_map(|result| async move { result })
            .take(limit)
            .collect()
            .await;

    // Sort by duration (fastest first)
    benchmarked_rpc_urls.sort_by_key(|(_, duration)| *duration);

    chain.benchmarked_rpc_urls = benchmarked_rpc_urls;

    Ok(chain)
}

pub async fn get_all_chains() -> Result<Vec<ChainInfo>> {
    let cache_dir = get_cache_dir();

    if let Ok(cached_data) = get_cached_chains(&cache_dir).await {
        return Ok(cached_data);
    }

    let client = reqwest::Client::new();
    let response = client.get(CHAINLIST_URL).send().await?;
    let chains: Vec<ChainInfo> = response.json().await?;

    if let Err(e) = cache_chains(&cache_dir, &chains).await {
        eprintln!("Warning: Failed to cache chains data: {e}");
    }

    Ok(chains)
}

fn get_cache_dir() -> std::path::PathBuf {
    let home_dir = home::home_dir().unwrap();
    home_dir.join(".mevlog").join(".chainlist-rpcs")
}

async fn get_cached_chains(cache_dir: &std::path::Path) -> Result<Vec<ChainInfo>> {
    let cached_data = cacache::read(cache_dir, CACHE_KEY).await?;
    let cache_info = cacache::metadata(cache_dir, CACHE_KEY)
        .await?
        .ok_or_else(|| eyre::eyre!("Cache metadata not found"))?;

    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let cache_time = cache_info.time;

    if now - cache_time > (CACHE_EXPIRY_SECONDS as u128 * 1000) {
        bail!("Cache expired");
    }

    let chains: Vec<ChainInfo> = serde_json::from_slice(&cached_data)?;
    Ok(chains)
}

async fn cache_chains(cache_dir: &std::path::Path, chains: &[ChainInfo]) -> Result<()> {
    let data = serde_json::to_vec(chains)?;
    cacache::write(cache_dir, CACHE_KEY, data).await?;
    Ok(())
}

pub async fn benchmark_url(url: String, timeout_ms: u64) -> Result<u64> {
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
        _ = sleep(Duration::from_millis(timeout_ms)) => {
            bail!("RPC URL timed out");
        }
    }
}
