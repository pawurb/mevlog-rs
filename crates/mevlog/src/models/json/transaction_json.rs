use revm::primitives::{Address, FixedBytes, U256};
use serde::{Deserialize, Serialize};

use crate::{
    db::txs::models::transaction::Transaction,
    misc::utils::{ETH_TRANSFER, GWEI_F64, wei_to_eth},
    models::json::log_json::LogJson,
};

/// TUI-ready JSON representation of a single transaction.
///
/// Display fields (`display_*`) are pre-formatted strings so consumers render
/// them directly. USD fields are populated only when a native token price is
/// available; coinbase/full-cost fields only when the tx was traced (i.e. its
/// `coinbase_transfer` is known).
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

    /// Gas cost (`gas_used * effective_gas_price`) in wei.
    pub tx_cost: u128,
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

impl TransactionJson {
    pub fn from_record(tx: &Transaction, native_token_price: Option<f64>) -> Self {
        let tx_cost: u128 = (tx.gas_used as u128) * tx.effective_gas_price;
        let tx_cost_u256 = U256::from(tx_cost);

        let (coinbase_transfer, display_coinbase_transfer, display_coinbase_transfer_usd) =
            match tx.coinbase_transfer {
                Some(cb) => (
                    Some(cb.to_string()),
                    Some(fmt_eth(cb)),
                    fmt_usd_opt(cb, native_token_price),
                ),
                None => (None, None, None),
            };

        let (full_tx_cost, display_full_tx_cost, display_full_tx_cost_usd) =
            match tx.coinbase_transfer {
                Some(cb) => {
                    let full = tx_cost_u256 + cb;
                    (
                        Some(full.to_string()),
                        Some(fmt_eth(full)),
                        fmt_usd_opt(full, native_token_price),
                    )
                }
                None => (None, None, None),
            };

        Self {
            block_number: tx.block_number,
            tx_index: tx.tx_index,
            tx_hash: tx.tx_hash,
            from: tx.from_address,
            to: tx.to_address,
            nonce: tx.nonce,
            signature: tx
                .signature
                .clone()
                .unwrap_or_else(|| ETH_TRANSFER.to_string()),
            signature_hash: tx.signature_hash.map(|h| format!("0x{}", hex::encode(h))),
            success: tx.success,
            value: tx.value.to_string(),
            display_value: fmt_eth(tx.value),
            gas_used: tx.gas_used,
            gas_price: tx.effective_gas_price,
            display_gas_price: fmt_gwei(tx.effective_gas_price),
            tx_cost,
            display_tx_cost: fmt_eth(tx_cost_u256),
            display_tx_cost_usd: fmt_usd_opt(tx_cost_u256, native_token_price),
            coinbase_transfer,
            display_coinbase_transfer,
            display_coinbase_transfer_usd,
            full_tx_cost,
            display_full_tx_cost,
            display_full_tx_cost_usd,
            logs: Vec::new(),
        }
    }
}

fn fmt_eth(wei: U256) -> String {
    format!("{:.6}", wei_to_eth(wei))
}

fn fmt_gwei(wei: u128) -> String {
    format!("{:.2}", wei as f64 / GWEI_F64)
}

fn fmt_usd_opt(wei: U256, native_token_price: Option<f64>) -> Option<String> {
    native_token_price.map(|price| format!("${:.2}", wei_to_eth(wei) * price))
}
