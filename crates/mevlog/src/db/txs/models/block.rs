use std::collections::HashSet;
use std::str::FromStr;

use arrow::record_batch::RecordBatch;
use eyre::Result;
use revm::primitives::{Address, FixedBytes};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use crate::misc::parquet_utils::get_parquet_string_value;

/// Per-block metadata, indexed alongside transactions during `query`.
///
/// A row exists for every indexed block (including empty ones), so the presence
/// of a row also marks the block as indexed (see [`Block::missing_blocks`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub block_number: u64,
    pub block_hash: FixedBytes<32>,
    /// Fee recipient (cryo `author`).
    pub miner: Address,
    pub gas_used: u64,
    /// Unix timestamp (seconds).
    pub timestamp: u64,
    /// `None` for pre-EIP-1559 blocks.
    pub base_fee_per_gas: Option<u64>,
}

#[hotpath::measure_all(future = true)]
impl Block {
    // Default cryo `blocks` columns: 0 block_hash, 1 author, 2 block_number,
    // 3 gas_used, 4 extra_data, 5 timestamp, 6 base_fee_per_gas.
    pub fn from_parquet_row(batch: &RecordBatch, row_idx: usize) -> Result<(Block, u64)> {
        let get = |col_idx: usize| -> String { get_parquet_string_value(batch, col_idx, row_idx) };

        let block_number = get(2).parse::<u64>().unwrap();

        let block = Block {
            block_number,
            block_hash: FixedBytes::<32>::from_str(&get(0)).unwrap(),
            miner: Address::from_str(&get(1)).unwrap(),
            gas_used: get(3).parse::<u64>().unwrap(),
            timestamp: get(5).parse::<u64>().unwrap(),
            base_fee_per_gas: get(6).parse::<u64>().ok(),
        };

        Ok((block, block_number))
    }

    pub async fn count(conn: &SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM blocks")
            .fetch_one(conn)
            .await?
            .get::<i64, _>(0);

        Ok(count)
    }

    pub async fn save<'c, E>(&self, executor: E) -> Result<()>
    where
        E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
    {
        sqlx::query(
            r#"
            INSERT INTO blocks (
                block_number, block_hash, miner, gas_used,
                timestamp, base_fee_per_gas
            ) VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(block_number) DO NOTHING
            "#,
        )
        .bind(self.block_number as i64)
        .bind(self.block_hash.as_slice())
        .bind(self.miner.as_slice())
        .bind(self.gas_used as i64)
        .bind(self.timestamp as i64)
        .bind(self.base_fee_per_gas.map(|v| v as i64))
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn save_batch(blocks: &[Block], conn: &SqlitePool) -> Result<()> {
        let mut db_tx = conn.begin().await?;

        for block in blocks {
            block.save(&mut *db_tx).await?;
        }

        db_tx.commit().await?;
        Ok(())
    }

    pub async fn missing_blocks(from: u64, to: u64, conn: &SqlitePool) -> Result<Vec<u64>> {
        let existing: Vec<i64> = sqlx::query_scalar(
            "SELECT block_number FROM blocks WHERE block_number BETWEEN ? AND ?",
        )
        .bind(from as i64)
        .bind(to as i64)
        .fetch_all(conn)
        .await?;

        let indexed: HashSet<u64> = existing.into_iter().map(|b| b as u64).collect();
        Ok((from..=to).filter(|b| !indexed.contains(b)).collect())
    }

    pub async fn query_where(where_sql: &str, conn: &SqlitePool) -> Result<Vec<Block>> {
        let sql = format!("SELECT * FROM blocks WHERE {where_sql} ORDER BY block_number DESC");

        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(conn)
            .await?;
        rows.iter().map(Self::from_row).collect()
    }

    fn from_row(row: &SqliteRow) -> Result<Block> {
        let block_number: i64 = row.try_get("block_number")?;
        let block_hash: Vec<u8> = row.try_get("block_hash")?;
        let miner: Vec<u8> = row.try_get("miner")?;
        let gas_used: i64 = row.try_get("gas_used")?;
        let timestamp: i64 = row.try_get("timestamp")?;
        let base_fee_per_gas: Option<i64> = row.try_get("base_fee_per_gas")?;

        Ok(Block {
            block_number: block_number as u64,
            block_hash: FixedBytes::<32>::from_slice(&block_hash),
            miner: Address::from_slice(&miner),
            gas_used: gas_used as u64,
            timestamp: timestamp as u64,
            base_fee_per_gas: base_fee_per_gas.map(|v| v as u64),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::txs::models::transaction::test::setup_test_db;

    fn sample_block(block_number: u64, base_fee_per_gas: Option<u64>) -> Block {
        Block {
            block_number,
            block_hash: FixedBytes::<32>::from([0xab; 32]),
            miner: Address::from([0x11; 20]),
            gas_used: 16_000_000,
            timestamp: 1_693_066_895,
            base_fee_per_gas,
        }
    }

    #[tokio::test]
    async fn save_batch_and_query_roundtrips() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let block = sample_block(100, Some(21_721_091_641));
        Block::save_batch(std::slice::from_ref(&block), &conn).await?;

        assert_eq!(Block::count(&conn).await?, 1);

        let found = Block::query_where("block_number = 100", &conn).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], block);

        Ok(())
    }

    #[tokio::test]
    async fn save_batch_is_idempotent() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let block = sample_block(100, None);
        Block::save_batch(std::slice::from_ref(&block), &conn).await?;
        Block::save_batch(std::slice::from_ref(&block), &conn).await?;

        assert_eq!(Block::count(&conn).await?, 1);
        Ok(())
    }

    #[tokio::test]
    async fn missing_blocks_excludes_indexed() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        // Empty blocks are still recorded, so 101 and 103 count as indexed.
        Block::save_batch(&[sample_block(101, None), sample_block(103, None)], &conn).await?;

        let missing = Block::missing_blocks(100, 104, &conn).await?;
        assert_eq!(missing, vec![100, 102, 104]);

        Ok(())
    }
}
