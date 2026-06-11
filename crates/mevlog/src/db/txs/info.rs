use eyre::Result;
use sqlx::{Row, SqlitePool};

/// Summary of the local txs DB contents. Block range fields are `None` when
/// the DB has no indexed blocks.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct DbInfoStats {
    pub blocks: u64,
    pub transactions: u64,
    pub logs: u64,
    pub min_block: Option<u64>,
    pub max_block: Option<u64>,
    pub min_block_timestamp: Option<u64>,
    pub max_block_timestamp: Option<u64>,
    /// Blocks absent from the indexed `min_block..=max_block` range.
    pub missing_blocks: u64,
}

/// Collects row counts and the indexed block range from the local txs DB.
pub async fn db_info(conn: &SqlitePool) -> Result<DbInfoStats> {
    let row = sqlx::query(
        "SELECT COUNT(*), MIN(block_number), MAX(block_number), MIN(timestamp), MAX(timestamp) FROM blocks",
    )
    .fetch_one(conn)
    .await?;

    let blocks: i64 = row.get(0);
    let min_block: Option<i64> = row.get(1);
    let max_block: Option<i64> = row.get(2);
    let min_block_timestamp: Option<i64> = row.get(3);
    let max_block_timestamp: Option<i64> = row.get(4);

    let transactions: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
        .fetch_one(conn)
        .await?;
    let logs: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
        .fetch_one(conn)
        .await?;

    let missing_blocks = match (min_block, max_block) {
        (Some(min), Some(max)) => (max - min + 1) as u64 - blocks as u64,
        _ => 0,
    };

    Ok(DbInfoStats {
        blocks: blocks as u64,
        transactions: transactions as u64,
        logs: logs as u64,
        min_block: min_block.map(|b| b as u64),
        max_block: max_block.map(|b| b as u64),
        min_block_timestamp: min_block_timestamp.map(|t| t as u64),
        max_block_timestamp: max_block_timestamp.map(|t| t as u64),
        missing_blocks,
    })
}
