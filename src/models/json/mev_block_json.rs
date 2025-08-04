use serde::Serialize;

use crate::models::{json::mev_transaction_json::MEVTransactionJson, mev_block::MEVBlock};

#[derive(Serialize)]
pub struct MEVBlockJson {
    pub block_number: u64,
    pub native_token_price: Option<f64>,
    pub transactions: Vec<MEVTransactionJson>,
    pub txs_count: u64,
}

impl From<&MEVBlock> for MEVBlockJson {
    fn from(block: &MEVBlock) -> Self {
        let mut mev_transactions = Vec::new();
        let mut tx_indices: Vec<_> = block.mev_transactions.keys().collect();
        tx_indices.sort();

        for &index in tx_indices {
            if let Some(tx) = block.mev_transactions.get(&index) {
                mev_transactions.push(MEVTransactionJson::from(tx));
            }
        }

        Self {
            block_number: block.block_number,
            native_token_price: block
                .native_token_price
                .map(|price| (price * 100.0).round() / 100.0),
            transactions: mev_transactions,
            txs_count: block.txs_count,
        }
    }
}
