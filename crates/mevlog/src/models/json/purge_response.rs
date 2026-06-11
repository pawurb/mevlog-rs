use serde::{Deserialize, Serialize};

use crate::{db::txs::purge::PurgeStats, models::json::query_response::format_duration};

/// Status envelope emitted by the `purge-db` command after removing indexed
/// data below the retained block window from the local txs DB. `latest_block`
/// (the highest indexed block) and `cutoff_block` (the lowest retained block)
/// are `null` when the DB was empty.
#[derive(Debug, Serialize, Deserialize)]
pub struct PurgeResponse {
    pub keep: u64,
    pub chain_id: u64,
    pub latest_block: Option<u64>,
    pub cutoff_block: Option<u64>,
    pub purged_blocks: u64,
    pub purged_transactions: u64,
    pub purged_logs: u64,
    pub duration: String,
}

impl PurgeResponse {
    pub fn new(keep: u64, chain_id: u64, stats: PurgeStats, duration_ns: u64) -> Self {
        Self {
            keep,
            chain_id,
            latest_block: stats.latest_block,
            cutoff_block: stats.cutoff_block,
            purged_blocks: stats.purged_blocks,
            purged_transactions: stats.purged_transactions,
            purged_logs: stats.purged_logs,
            duration: format_duration(duration_ns),
        }
    }
}

/// Serializes a `PurgeResponse` as JSON (pretty when requested).
pub fn serialize_purge_response(resp: &PurgeResponse, pretty: bool) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(resp)
    } else {
        serde_json::to_string(resp)
    }
}
