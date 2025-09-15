use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use alloy::{sol, uint};
use eyre::Result;
use revm::primitives::U256;

use crate::{GenericProvider, models::evm_chain::EVMChain};

pub const SEPARATORER: &str = "===============================================================================================";
pub const SEPARATOR: &str = "-----------------------------------------------------------------------------------------------";

pub const ETH_TRANSFER: &str = "<ETH transfer>";
pub const UNKNOWN: &str = "<Unknown>";

pub const ETHER: U256 = uint!(1_000_000_000_000_000_000_U256);
pub const GWEI: U256 = uint!(1_000_000_000_U256);
pub const GWEI_U128: u128 = 1_000_000_000_u128;
pub const GWEI_F64: f64 = 1_000_000_000_f64;

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

pub fn init_logs() {
    #[cfg(not(feature = "tokio-console"))]
    {
        let offset = ::time::UtcOffset::from_hms(1, 0, 0).expect("should get CET offset");
        let time_format =
            time::format_description::parse("[year]-[month]-[day]T[hour]:[minute]:[second]")
                .unwrap();
        let timer = tracing_subscriber::fmt::time::OffsetTime::new(offset, time_format);

        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .with_timer(timer)
            .init();
    }

    #[cfg(feature = "tokio-console")]
    {
        console_subscriber::init();
    }
}

pub fn measure_start(label: &str) -> (String, Instant) {
    (label.to_string(), Instant::now())
}

pub fn measure_end(start: (String, Instant)) -> Duration {
    let elapsed = start.1.elapsed();
    tracing::info!("Elapsed: {:.2?} for '{}'", elapsed, start.0);
    elapsed
}

pub trait ToU64 {
    fn to_u64(&self) -> u64;
}

impl ToU64 for U256 {
    fn to_u64(&self) -> u64 {
        U256::to::<u64>(self)
    }
}

pub trait ToU128 {
    fn to_u128(&self) -> u128;
}

impl ToU128 for U256 {
    fn to_u128(&self) -> u128 {
        U256::to::<u128>(self)
    }
}

pub fn wei_to_eth(wei: U256) -> f64 {
    let wei_per_eth = ETHER;
    let wei_f64 = wei.to_string().parse::<f64>().unwrap();
    let wei_per_eth_f64 = wei_per_eth.to_string().parse::<f64>().unwrap();

    wei_f64 / wei_per_eth_f64
}

pub async fn get_native_token_price(
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
            println!("Error getting native token price: {e:?}");
            return Ok(None);
        }
    };
    let native_token_price = native_token_price.low_i64() as f64 / 10e7;
    Ok(Some(native_token_price))
}
