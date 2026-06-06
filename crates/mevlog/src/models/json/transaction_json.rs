use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Deserializer, Serialize};

use crate::models::json::log_json::LogJson;

/// JSON representation of a single transaction; the deserialization contract for
/// the `mevlog tx` output (rendered by [`tx_display_query`]).
///
/// USD fields are populated only when a native token price is available;
/// coinbase/full-cost fields only when the tx was traced.
///
/// [`tx_display_query`]: crate::db::txs::display_sql::tx_display_query
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TransactionJson {
    pub block_number: u64,
    pub tx_index: u64,
    pub tx_hash: FixedBytes<32>,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub signature: String,
    pub signature_hash: Option<String>,
    /// SQLite has no boolean type, so `success` arrives as `0`/`1`.
    #[serde(deserialize_with = "bool_from_int")]
    pub success: bool,

    /// Raw value in wei as a decimal string.
    pub value: String,
    /// Value formatted in ETH.
    pub display_value: String,

    pub gas_used: u64,
    /// Effective gas price in wei.
    pub gas_price: u128,
    /// Gas price formatted in gwei.
    pub display_gas_price: String,

    /// Gas cost (`gas_used * effective_gas_price`) in wei as a decimal string.
    pub tx_cost: String,
    /// Gas cost formatted in ETH.
    pub display_tx_cost: String,
    pub display_tx_cost_usd: Option<String>,

    /// Direct ETH paid to the block coinbase, in wei as a decimal string.
    pub coinbase_transfer: Option<String>,
    pub display_coinbase_transfer: Option<String>,
    pub display_coinbase_transfer_usd: Option<String>,

    /// Gas cost plus coinbase transfer, in wei as a decimal string.
    pub full_tx_cost: Option<String>,
    pub display_full_tx_cost: Option<String>,
    pub display_full_tx_cost_usd: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<LogJson>,
}

/// Accepts a JSON boolean or the `0`/`1` integer SQLite stores for booleans.
fn bool_from_int<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum BoolOrInt {
        Bool(bool),
        Int(i64),
    }

    Ok(match BoolOrInt::deserialize(deserializer)? {
        BoolOrInt::Bool(b) => b,
        BoolOrInt::Int(n) => n != 0,
    })
}
