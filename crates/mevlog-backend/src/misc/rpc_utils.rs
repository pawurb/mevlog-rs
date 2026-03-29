use eyre::{Result, bail};
use mevlog::ChainInfoJson;
use rand::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command as AsyncCommand;
use tokio::sync::RwLock;

use crate::controllers::json::base_controller::call_json_command;

#[derive(Clone)]
struct CachedRpcUrls {
    urls: Vec<String>,
    cached_at: Instant,
}

type RpcCache = Arc<RwLock<HashMap<u64, CachedRpcUrls>>>;
static RPC_URL_MEMORY_CACHE: std::sync::LazyLock<RpcCache> =
    std::sync::LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));
const CACHE_DURATION: Duration = Duration::from_secs(60); // 1 minute

#[hotpath::measure(log = true)]
pub async fn get_random_rpc_url(chain_id: u64) -> Result<Option<String>> {
    let urls = get_cached_rpc_urls(chain_id).await?;
    let mut rng = rand::rng();
    Ok(urls.choose(&mut rng).cloned())
}

#[hotpath::measure(log = true)]
async fn get_cached_rpc_urls(chain_id: u64) -> Result<Vec<String>> {
    {
        let cache_read = RPC_URL_MEMORY_CACHE.read().await;
        if let Some(cached) = cache_read.get(&chain_id)
            && cached.cached_at.elapsed() < CACHE_DURATION
        {
            return Ok(cached.urls.clone());
        }
    }

    let chain_info = fetch_chain_info(chain_id).await?;
    let top_rpc_urls: Vec<String> = chain_info
        .rpc_urls
        .into_iter()
        .take(3)
        .map(|rpc| rpc.url)
        .collect();

    {
        let mut cache_write = RPC_URL_MEMORY_CACHE.write().await;
        cache_write.insert(
            chain_id,
            CachedRpcUrls {
                urls: top_rpc_urls.clone(),
                cached_at: Instant::now(),
            },
        );
    }

    Ok(top_rpc_urls)
}

#[hotpath::measure(log = true)]
async fn fetch_chain_info(chain_id: u64) -> Result<ChainInfoJson> {
    let mut cmd = AsyncCommand::new("mevlog");
    cmd.arg("chain-info")
        .arg("--chain-id")
        .arg(chain_id.to_string())
        .arg("--format")
        .arg("json");

    match call_json_command::<ChainInfoJson>(&mut cmd).await {
        Ok(chain_info) => Ok(chain_info),
        Err(e) => bail!("Failed to get chain info for chain_id {chain_id}: {e}",),
    }
}
