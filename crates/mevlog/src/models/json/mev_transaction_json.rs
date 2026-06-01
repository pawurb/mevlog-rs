use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::models::transaction::TransactionJson,
    misc::shared_init::TraceMode,
    models::{
        json::{
            mev_log_group_json::MEVLogGroupJson, mev_opcode_json::MEVOpcodeJson,
            mev_state_diff_json::MEVStateDiffJson,
        },
        mev_transaction::CallExtract,
    },
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

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Serialize)]
pub struct QueryParams {
    pub command: &'static str,
    pub blocks: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evm_trace: Option<TraceMode>,
    #[serde(skip_serializing_if = "is_false")]
    pub evm_calls: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub evm_ops: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub evm_state_diff: bool,
}

pub fn format_duration(ns: u64) -> String {
    if ns < 1_000 {
        format!("{} ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.2} µs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.2} s", ns as f64 / 1_000_000_000.0)
    }
}

#[derive(Serialize)]
struct QueryResponseEnvelopeJson<'a, Q: Serialize> {
    result: &'a [TransactionJson],
    result_count: usize,
    cached_blocks: u64,
    new_blocks: u64,
    duration: String,
    chain: &'a ChainInfoNoRpcsJson,
    query: Q,
}

/// Serializes barebones [`TransactionJson`] rows (no logs/traces) into the
/// standard response envelope used by the SQLite-backed query path.
pub fn serialize_query_response<Q: Serialize>(
    transactions: &[TransactionJson],
    pretty: bool,
    chain: &ChainInfoNoRpcsJson,
    duration_ns: u64,
    cached_blocks: u64,
    new_blocks: u64,
    query: Q,
) -> serde_json::Result<String> {
    let envelope = QueryResponseEnvelopeJson {
        result: transactions,
        result_count: transactions.len(),
        cached_blocks,
        new_blocks,
        duration: format_duration(duration_ns),
        chain,
        query,
    };

    if pretty {
        serde_json::to_string_pretty(&envelope)
    } else {
        serde_json::to_string(&envelope)
    }
}
