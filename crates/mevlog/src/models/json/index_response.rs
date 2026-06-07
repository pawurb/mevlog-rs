use serde::{Deserialize, Serialize};

use crate::{ChainInfoNoRpcsJson, models::json::query_response::format_duration};

/// Status envelope emitted by the `index` command after indexing a block range
/// into the local txs DB. Reports block counts and timing only (no rows).
#[derive(Debug, Serialize, Deserialize)]
pub struct IndexResponse {
    pub blocks: String,
    pub from: u64,
    pub to: u64,
    pub total_blocks: u64,
    pub new_blocks: u64,
    pub cached_blocks: u64,
    pub duration: String,
    pub chain: ChainInfoNoRpcsJson,
}

impl IndexResponse {
    pub fn new(
        blocks: String,
        from: u64,
        to: u64,
        cached_blocks: u64,
        new_blocks: u64,
        duration_ns: u64,
        chain: ChainInfoNoRpcsJson,
    ) -> Self {
        Self {
            blocks,
            from,
            to,
            total_blocks: cached_blocks + new_blocks,
            new_blocks,
            cached_blocks,
            duration: format_duration(duration_ns),
            chain,
        }
    }
}

/// Serializes an `IndexResponse` as JSON (pretty when requested).
pub fn serialize_index_response(resp: &IndexResponse, pretty: bool) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(resp)
    } else {
        serde_json::to_string(resp)
    }
}
