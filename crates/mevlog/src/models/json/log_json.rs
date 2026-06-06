use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

use crate::db::txs::models::log::Log;

/// TUI-ready JSON representation of a single log/event row.
///
/// Flat (no grouping). `erc20_amount` is the raw token-base-units value as a
/// decimal string; human-readable formatting needs the token's decimals/symbol,
/// which is resolved separately, so it is intentionally left raw here.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LogJson {
    pub log_index: u64,
    pub address: Address,
    /// Resolved event signature, `None` when it could not be resolved.
    pub signature: Option<String>,
    pub topics: Vec<FixedBytes<32>>,
    /// Raw log data as `0x`-hex.
    pub data: String,
    /// Decoded ERC20 transfer amount as a decimal string, `None` for non-transfer logs.
    pub erc20_amount: Option<String>,
}

impl LogJson {
    pub fn from_record(log: &Log) -> Self {
        Self {
            log_index: log.log_index,
            address: log.address,
            signature: log.signature.clone(),
            topics: log.topics.clone(),
            data: format!("0x{}", hex::encode(&log.data)),
            erc20_amount: log.erc20_amount.map(|a| a.to_string()),
        }
    }
}
