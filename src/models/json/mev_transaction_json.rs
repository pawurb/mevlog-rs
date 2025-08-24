use revm::primitives::{Address, FixedBytes, TxKind, U256};
use serde::Serialize;

use crate::{
    misc::utils::ToU128,
    models::{
        json::mev_log_group_json::MEVLogGroupJson,
        mev_transaction::{
            calculate_create_address, display_token, display_token_and_usd, display_usd,
            eth_to_usd, CallExtract, MEVTransaction,
        },
    },
};

#[derive(Serialize)]
pub struct MEVTransactionJson {
    pub block_number: u64,
    pub signature: String,
    pub signature_hash: Option<String>,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    pub from: Address,
    pub from_ens: Option<String>,
    pub to: Option<Address>,
    pub nonce: u64,
    pub value: String,
    pub display_value: String,
    pub coinbase_transfer: Option<String>,
    pub display_coinbase_transfer: Option<String>,
    pub display_coinbase_transfer_usd: Option<String>,
    pub success: bool,
    pub gas_price: u128,
    pub gas_used: u64,
    pub tx_cost: u128,
    pub display_tx_cost: String,
    pub display_tx_cost_usd: Option<String>,
    pub full_tx_cost: Option<u128>,
    pub display_full_tx_cost: Option<String>,
    pub display_full_tx_cost_usd: Option<String>,
    pub calls: Option<Vec<CallExtract>>,
    pub log_groups: Vec<MEVLogGroupJson>,
}

impl From<&MEVTransaction> for MEVTransactionJson {
    fn from(tx: &MEVTransaction) -> Self {
        let log_groups = tx.log_groups().iter().map(MEVLogGroupJson::from).collect();

        let gas_tx_cost = tx.receipt.gas_used as u128 * tx.receipt.effective_gas_price;
        let full_tx_cost = tx.full_tx_cost().map(|amt| amt.to_u128());

        let to = match tx.to {
            TxKind::Create => Some(calculate_create_address(tx.nonce, tx.from())),
            TxKind::Call(address) => Some(address),
        };

        Self {
            block_number: tx.block_number,
            signature: tx.signature.clone(),
            signature_hash: tx.signature_hash.clone(),
            tx_hash: tx.tx_hash,
            index: tx.index,
            from: tx.from(),
            from_ens: tx.ens_name().map(|s| s.to_string()),
            to,
            nonce: tx.nonce,
            value: tx.value().to_string(),
            coinbase_transfer: tx.coinbase_transfer.map(|amt| amt.to_string()),
            display_coinbase_transfer: tx
                .coinbase_transfer
                .map(|amt| display_token(amt, &tx.chain.currency_symbol, false)),
            display_coinbase_transfer_usd: tx.coinbase_transfer.and_then(|amt| {
                tx.native_token_price
                    .map(|price| display_usd(eth_to_usd(amt, price)))
            }),
            success: tx.receipt.success,
            gas_price: tx.receipt.effective_gas_price,
            tx_cost: gas_tx_cost,
            display_tx_cost: display_token(
                U256::from(gas_tx_cost),
                &tx.chain.currency_symbol,
                false,
            ),
            display_tx_cost_usd: tx
                .native_token_price
                .map(|price| display_usd(eth_to_usd(U256::from(gas_tx_cost), price))),
            display_value: display_token_and_usd(
                tx.value(),
                tx.native_token_price,
                &tx.chain.currency_symbol,
            ),
            full_tx_cost,
            display_full_tx_cost: full_tx_cost
                .map(|amt| display_token(U256::from(amt), &tx.chain.currency_symbol, false)),
            display_full_tx_cost_usd: full_tx_cost.and_then(|amt| {
                tx.native_token_price
                    .map(|price| display_usd(eth_to_usd(U256::from(amt), price)))
            }),
            gas_used: tx.receipt.gas_used,
            calls: tx.calls.clone(),
            log_groups,
        }
    }
}
