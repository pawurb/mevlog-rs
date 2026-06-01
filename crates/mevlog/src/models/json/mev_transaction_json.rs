use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

use crate::models::{
    json::{
        mev_log_group_json::MEVLogGroupJson, mev_opcode_json::MEVOpcodeJson,
        mev_state_diff_json::MEVStateDiffJson,
    },
    mev_transaction::CallExtract,
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MEVTransactionJson {
    pub block_number: u64,
    pub signature: String,
    pub signature_hash: Option<String>,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    pub from: Address,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evm_calls: Vec<CallExtract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<MEVLogGroupJson>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evm_opcodes: Vec<MEVOpcodeJson>,
    #[serde(default, skip_serializing_if = "MEVStateDiffJson::is_empty")]
    pub evm_state_diff: MEVStateDiffJson,
}
