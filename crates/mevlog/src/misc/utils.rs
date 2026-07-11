use std::sync::Arc;

use alloy::{sol, uint};
use eyre::Result;
use revm::primitives::U256;

use crate::{GenericProvider, models::evm_chain::EVMChain};

pub const ETH_TRANSFER: &str = "<ETH transfer>";

const ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub const GWEI_F64: f64 = 1_000_000_000_f64;

pub(crate) fn block_cache_key(chain: &EVMChain, block_number: u64) -> String {
    format!("{}-{}", chain.name, block_number)
}

sol! {
    #[sol(rpc)]
    contract IPriceOracle {
    function latestRoundData()
        returns (
        uint80 roundId,
        int256 answer,
        uint256 startedAt,
        uint256 updatedAt,
        uint80 answeredInRound
        );
    }
}

pub fn init_std_logs() {
    init_logs_inner(false);
}

pub fn init_file_logs() {
    init_logs_inner(true);
}

fn init_logs_inner(to_file: bool) {
    #[cfg(not(feature = "tokio-console"))]
    {
        use tracing_subscriber::{Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

        let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
        let time_format = time::format_description::parse_borrowed::<2>(
            "[year]-[month]-[day]T[hour]:[minute]:[second]",
        )
        .unwrap();
        let timer = tracing_subscriber::fmt::time::OffsetTime::new(offset, time_format);
        // The EnvFilter is attached per-layer, not to the registry: a global
        // filter would suppress the `sqlx::query` events that hotpath's SQL
        // tracing layer listens for.
        let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("error"));

        if to_file {
            std::fs::create_dir_all("log").expect("failed to create log directory");
            let log_file =
                std::fs::File::create("log/development.log").expect("failed to create log file");
            let file_layer = fmt::layer()
                .with_writer(log_file)
                .with_ansi(false)
                .with_timer(timer)
                .with_target(false)
                .with_thread_ids(false)
                .with_filter(env_filter);

            tracing_subscriber::registry()
                .with(hotpath::sqlx_tracing_layer())
                .with(file_layer)
                .init();
        } else {
            let stderr_layer = fmt::layer()
                .with_writer(std::io::stderr)
                .with_timer(timer)
                .with_filter(env_filter);

            tracing_subscriber::registry()
                .with(hotpath::sqlx_tracing_layer())
                .with(stderr_layer)
                .init();
        }
    }

    #[cfg(feature = "tokio-console")]
    {
        let _ = to_file; // suppress unused warning
        console_subscriber::init();
    }
}

pub(crate) fn wei_to_eth(wei: U256) -> f64 {
    let wei_f64 = wei.to_string().parse::<f64>().unwrap();
    let wei_per_eth_f64 = ETHER.to_string().parse::<f64>().unwrap();
    wei_f64 / wei_per_eth_f64
}

pub(crate) async fn get_native_token_price(
    chain: &EVMChain,
    provider: &Arc<GenericProvider>,
    native_token_price: Option<f64>,
) -> Result<Option<f64>> {
    if let Some(native_token_price) = native_token_price {
        return Ok(Some(native_token_price));
    }

    if chain.chainlink_oracle.is_none() {
        return Ok(None);
    }

    let price_oracle = IPriceOracle::new(chain.chainlink_oracle.unwrap(), provider.clone());
    let native_token_price = match price_oracle.latestRoundData().call().await {
        Ok(price) => price.answer,
        Err(e) => {
            tracing::warn!("Error getting native token price: {e:?}");
            return Ok(None);
        }
    };
    let native_token_price = native_token_price.low_i64() as f64 / 10e7;
    Ok(Some(native_token_price))
}
