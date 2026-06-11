use chrono::DateTime;
use serde::{Deserialize, Serialize};

use crate::db::txs::info::DbInfoStats;

/// Summary envelope emitted by the `db-info` command. Block range and
/// timestamp fields are `null` when the DB has no indexed blocks.
#[derive(Debug, Serialize, Deserialize)]
pub struct DbInfoResponse {
    pub chain_id: u64,
    pub db_path: String,
    pub schema_version: u64,
    pub db_size: String,
    pub db_size_bytes: u64,
    pub wal_size_bytes: u64,
    pub blocks: u64,
    pub transactions: u64,
    pub logs: u64,
    pub min_block: Option<u64>,
    pub max_block: Option<u64>,
    pub min_block_timestamp: Option<u64>,
    pub max_block_timestamp: Option<u64>,
    pub min_block_time: Option<String>,
    pub max_block_time: Option<String>,
    pub missing_blocks: u64,
}

impl DbInfoResponse {
    pub fn new(
        chain_id: u64,
        db_path: String,
        schema_version: u64,
        db_size_bytes: u64,
        wal_size_bytes: u64,
        stats: DbInfoStats,
    ) -> Self {
        Self {
            chain_id,
            db_path,
            schema_version,
            db_size: format_size(db_size_bytes),
            db_size_bytes,
            wal_size_bytes,
            blocks: stats.blocks,
            transactions: stats.transactions,
            logs: stats.logs,
            min_block: stats.min_block,
            max_block: stats.max_block,
            min_block_timestamp: stats.min_block_timestamp,
            max_block_timestamp: stats.max_block_timestamp,
            min_block_time: stats.min_block_timestamp.map(format_timestamp),
            max_block_time: stats.max_block_timestamp.map(format_timestamp),
            missing_blocks: stats.missing_blocks,
        }
    }
}

fn format_timestamp(ts: u64) -> String {
    match DateTime::from_timestamp(ts as i64, 0) {
        Some(dt) => dt.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
        None => ts.to_string(),
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{size:.2} {}", UNITS[unit])
    }
}

/// Serializes a `DbInfoResponse` as JSON (pretty when requested).
pub fn serialize_db_info_response(
    resp: &DbInfoResponse,
    pretty: bool,
) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(resp)
    } else {
        serde_json::to_string(resp)
    }
}
