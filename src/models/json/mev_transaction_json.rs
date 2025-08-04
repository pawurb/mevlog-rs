use revm::primitives::{Address, FixedBytes};
use serde::Serialize;

use crate::models::{
    json::mev_log_group_json::MEVLogGroupJson,
    mev_transaction::{CallExtract, MEVTransaction},
};

#[derive(Serialize)]
pub struct MEVTransactionJson {
    pub signature: String,
    pub signature_hash: Option<String>,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub nonce: u64,
    pub value: String,
    pub coinbase_transfer: Option<String>,
    pub success: bool,
    pub effective_gas_price: u128,
    pub gas_used: u64,
    pub calls: Option<Vec<CallExtract>>,
    pub log_groups: Vec<MEVLogGroupJson>,
}

impl From<&MEVTransaction> for MEVTransactionJson {
    fn from(tx: &MEVTransaction) -> Self {
        let log_groups = tx.log_groups().iter().map(MEVLogGroupJson::from).collect();

        Self {
            signature: tx.signature.clone(),
            signature_hash: tx.signature_hash.clone(),
            tx_hash: tx.tx_hash,
            index: tx.index,
            from: tx.from(),
            to: tx.to(),
            nonce: tx.nonce,
            value: tx.value().to_string(),
            coinbase_transfer: tx.coinbase_transfer.map(|amt| amt.to_string()),
            success: tx.receipt.success,
            effective_gas_price: tx.receipt.effective_gas_price,
            gas_used: tx.receipt.gas_used,
            calls: tx.calls.clone(),
            log_groups,
        }
    }
}
