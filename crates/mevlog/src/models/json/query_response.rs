use serde::Serialize;

use crate::{ChainInfoNoRpcsJson, misc::shared_init::TraceMode};

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Serialize)]
pub struct QueryParams {
    pub command: &'static str,
    pub blocks: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
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
    result: &'a [serde_json::Value],
    result_count: usize,
    cached_blocks: u64,
    new_blocks: u64,
    duration: String,
    chain: &'a ChainInfoNoRpcsJson,
    query: Q,
}

/// Serializes generic SQL result rows into the standard response envelope used by
/// the SQLite-backed query path.
pub fn serialize_query_response<Q: Serialize>(
    results: &[serde_json::Value],
    pretty: bool,
    chain: &ChainInfoNoRpcsJson,
    duration_ns: u64,
    cached_blocks: u64,
    new_blocks: u64,
    query: Q,
) -> serde_json::Result<String> {
    let envelope = QueryResponseEnvelopeJson {
        result: results,
        result_count: results.len(),
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
