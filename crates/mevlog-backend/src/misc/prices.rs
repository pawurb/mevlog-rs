use std::sync::LazyLock;
use std::time::Duration;

use eyre::{Result, bail};
use reqwest;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

const COINGECKO_API_URL: &str =
    "https://api.coingecko.com/api/v3/simple/price?ids=ethereum,binancecoin&vs_currencies=usd";

static PRICE_CACHE: LazyLock<RwLock<Option<PriceResponse>>> = LazyLock::new(|| RwLock::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenPrice {
    #[serde(default)]
    pub usd: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceResponse {
    pub ethereum: TokenPrice,
    pub binancecoin: TokenPrice,
}

pub(crate) async fn get_price_for_chain_id(chain_id: u64) -> Result<Option<f64>> {
    match chain_id {
        1 => Ok(get_crypto_prices().await?.ethereum.usd),
        56 => Ok(get_crypto_prices().await?.binancecoin.usd),
        _ => Ok(None),
    }
}

pub(crate) async fn get_crypto_prices() -> Result<PriceResponse> {
    {
        let cache = PRICE_CACHE.read().await;
        if let Some(cached_prices) = cache.as_ref() {
            return Ok(cached_prices.clone());
        }
    }

    let prices = fetch_prices_from_api().await?;

    {
        let mut cache = PRICE_CACHE.write().await;
        tracing::debug!("Prices cache updated: {:?}", prices);
        *cache = Some(prices.clone());
    }

    Ok(prices)
}

pub(crate) async fn update_prices_cache() -> Result<()> {
    let prices = fetch_prices_from_api().await?;
    {
        let mut cache = PRICE_CACHE.write().await;
        tracing::debug!("Prices cache updated: {:?}", prices);
        *cache = Some(prices);
    }

    Ok(())
}

/// Warms the process-local price cache once and spawns a background task that
/// refreshes it every 5 minutes. Must run inside the server process, since the
/// cache is a process-local static and only the server's request handlers read
/// it (`get_price_for_chain_id`). A warm cache lets web queries pass
/// `--native-token-price`, avoiding the per-request Chainlink oracle RPC.
pub async fn spawn_price_refresh() {
    const REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

    if let Err(e) = update_prices_cache().await {
        tracing::error!("Failed to warm prices cache at startup: {}", &e);
    }

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(REFRESH_INTERVAL);
        interval.tick().await; // first tick fires immediately; skip (just warmed)
        loop {
            interval.tick().await;
            match update_prices_cache().await {
                Ok(_) => tracing::info!("Prices cache updated"),
                Err(e) => tracing::error!("Failed to update prices cache: {}", &e),
            }
        }
    });
}

async fn fetch_prices_from_api() -> Result<PriceResponse> {
    let client = reqwest::Client::new();
    let response = match client
        .get(COINGECKO_API_URL)
        .header("User-Agent", "mevlog-backend/1.0")
        .send()
        .await
    {
        Ok(response) => response,
        Err(e) => {
            let msg = format!("Failed to fetch prices from API: {}", e);
            tracing::error!("{}", &msg);
            bail!("{}", &msg)
        }
    };

    let status = response.status();
    let body = response.text().await?;

    if !status.is_success() {
        tracing::error!("CoinGecko API error ({}): {}", status, &body);
        bail!("CoinGecko API returned status {}: {}", status, &body);
    }

    match serde_json::from_str::<PriceResponse>(&body) {
        Ok(prices) => Ok(prices),
        Err(e) => {
            tracing::error!(
                "Failed to parse CoinGecko response: {} | Body: {}",
                &e,
                &body
            );
            bail!("Failed to parse prices: {}", e)
        }
    }
}
