use serde::Serialize;

use crate::{
    misc::utils::ToU64,
    models::mev_block::{format_block_age, MEVBlock},
};

#[derive(Serialize)]
pub struct MEVBlockJson {
    pub block_number: u64,
    pub native_token_price: Option<f64>,
    pub all_txs_count: u64,
    pub display_age: String,
    pub display_base_fee: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub explorer_url: Option<String>,
}

impl From<&MEVBlock> for MEVBlockJson {
    fn from(block: &MEVBlock) -> Self {
        let timestamp = block.revm_context.timestamp;
        let age = chrono::Utc::now().timestamp() - timestamp as i64;
        let base_fee_gwei = block.revm_context.basefee.to_u64() as f64 / 1000000000.0;

        Self {
            block_number: block.block_number,
            native_token_price: block
                .native_token_price
                .map(|price| (price * 100.0).round() / 100.0),
            all_txs_count: block.txs_count,
            display_age: format_block_age(age),
            display_base_fee: format!("{base_fee_gwei:.2} gwei"),
            chain_id: block.chain.chain_id,
            chain_name: block.chain.name.to_string(),
            explorer_url: block.chain.explorer_url.clone(),
        }
    }
}
