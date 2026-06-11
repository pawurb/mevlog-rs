use eyre::Result;
use sqlx::SqlitePool;

use crate::db::shared::truncate_wal;

/// Row counts removed by [`purge_old_blocks`]. `latest_block`/`cutoff_block`
/// are `None` when the DB had no indexed blocks (nothing to purge).
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PurgeStats {
    /// Highest indexed block, used as the reference for the cutoff.
    pub latest_block: Option<u64>,
    pub cutoff_block: Option<u64>,
    pub purged_blocks: u64,
    pub purged_transactions: u64,
    pub purged_logs: u64,
}

/// Deletes all indexed data below a `keep`-sized block-number window ending at
/// the newest indexed block. With gapless indexing this equals keeping the
/// `keep` newest indexed blocks; with disjoint indexed ranges, older islands
/// falling outside the window are purged regardless of how many blocks are
/// indexed in total.
///
/// The highest block present in the local DB is the reference (no RPC calls):
/// rows with `block_number < MAX(blocks.block_number) - keep + 1` are removed
/// from `logs`, `transactions`, and `blocks` in a single transaction
/// (`keep = 0` purges everything). Disk space is reclaimed afterwards via
/// `VACUUM` plus a WAL truncation.
pub async fn purge_old_blocks(keep: u64, conn: &SqlitePool) -> Result<PurgeStats> {
    let latest_block: Option<i64> = sqlx::query_scalar("SELECT MAX(block_number) FROM blocks")
        .fetch_one(conn)
        .await?;

    let Some(latest_block) = latest_block else {
        return Ok(PurgeStats::default());
    };
    let latest_block = latest_block as u64;
    let cutoff_block = latest_block.saturating_add(1).saturating_sub(keep);

    let mut db_tx = conn.begin().await?;

    let purged_logs = sqlx::query("DELETE FROM logs WHERE block_number < ?")
        .bind(cutoff_block as i64)
        .execute(&mut *db_tx)
        .await?
        .rows_affected();

    let purged_transactions = sqlx::query("DELETE FROM transactions WHERE block_number < ?")
        .bind(cutoff_block as i64)
        .execute(&mut *db_tx)
        .await?
        .rows_affected();

    let purged_blocks = sqlx::query("DELETE FROM blocks WHERE block_number < ?")
        .bind(cutoff_block as i64)
        .execute(&mut *db_tx)
        .await?
        .rows_affected();

    db_tx.commit().await?;

    sqlx::query("VACUUM").execute(conn).await?;
    truncate_wal(conn).await?;

    Ok(PurgeStats {
        latest_block: Some(latest_block),
        cutoff_block: Some(cutoff_block),
        purged_blocks,
        purged_transactions,
        purged_logs,
    })
}

#[cfg(test)]
mod test {
    use revm::primitives::{Address, FixedBytes, U256};

    use super::*;
    use crate::db::txs::models::{
        block::Block,
        log::Log,
        transaction::{Transaction, test::setup_test_db},
    };

    fn sample_block(block_number: u64) -> Block {
        Block {
            block_number,
            block_hash: FixedBytes::<32>::from([0xab; 32]),
            miner: Address::from([0x11; 20]),
            gas_used: 16_000_000,
            timestamp: 1_693_066_895,
            base_fee_per_gas: None,
        }
    }

    fn sample_tx(block_number: u64) -> Transaction {
        Transaction {
            block_number,
            tx_index: 0,
            tx_hash: FixedBytes::<32>::from([block_number as u8; 32]),
            nonce: 7,
            from_address: Address::from([0x11; 20]),
            to_address: Some(Address::from([0x22; 20])),
            value: U256::from(1u64),
            gas_limit: 21_000,
            gas_used: 21_000,
            effective_gas_price: 30_000_000_000,
            gas_price: 30_000_000_000,
            max_fee_per_gas: 40_000_000_000,
            max_priority_fee_per_gas: 2_000_000_000,
            transaction_type: Some(2),
            success: true,
            signature_hash: None,
            signature: None,
            coinbase_transfer: None,
        }
    }

    fn sample_log(block_number: u64) -> Log {
        Log {
            block_number,
            tx_index: 0,
            log_index: 0,
            address: Address::from([0x11; 20]),
            topics: vec![FixedBytes::<32>::from([0xdd; 32])],
            data: vec![0xde, 0xad],
            erc20_amount: None,
            signature: None,
        }
    }

    async fn seed_blocks(range: std::ops::RangeInclusive<u64>, conn: &SqlitePool) -> Result<()> {
        let blocks: Vec<Block> = range.clone().map(sample_block).collect();
        let txs: Vec<Transaction> = range.clone().map(sample_tx).collect();
        let logs: Vec<Log> = range.map(sample_log).collect();

        Block::save_batch(&blocks, conn).await?;
        Transaction::save_batch(&txs, conn).await?;
        Log::save_batch(&logs, conn).await?;
        Ok(())
    }

    #[tokio::test]
    async fn purge_removes_data_older_than_keep_newest_blocks() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_blocks(100..=104, &conn).await?;

        let stats = purge_old_blocks(2, &conn).await?;

        assert_eq!(
            stats,
            PurgeStats {
                latest_block: Some(104),
                cutoff_block: Some(103),
                purged_blocks: 3,
                purged_transactions: 3,
                purged_logs: 3,
            }
        );

        let blocks = Block::query_where("1 = 1", &conn).await?;
        let remaining: Vec<u64> = blocks.iter().map(|b| b.block_number).collect();
        assert_eq!(remaining, vec![104, 103]);

        assert!(
            Transaction::query_where("block_number < 103", &conn)
                .await?
                .is_empty()
        );
        assert!(
            Log::query_where("block_number < 103", &conn)
                .await?
                .is_empty()
        );

        Ok(())
    }

    #[tokio::test]
    async fn purge_keeps_everything_when_keep_covers_all_blocks() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_blocks(100..=104, &conn).await?;

        let stats = purge_old_blocks(1000, &conn).await?;

        assert_eq!(stats.latest_block, Some(104));
        assert_eq!(stats.cutoff_block, Some(0));
        assert_eq!(stats.purged_blocks, 0);
        assert_eq!(stats.purged_transactions, 0);
        assert_eq!(stats.purged_logs, 0);
        assert_eq!(Block::count(&conn).await?, 5);

        Ok(())
    }

    #[tokio::test]
    async fn purge_with_keep_zero_removes_everything() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_blocks(100..=104, &conn).await?;

        let stats = purge_old_blocks(0, &conn).await?;

        assert_eq!(stats.cutoff_block, Some(105));
        assert_eq!(stats.purged_blocks, 5);
        assert_eq!(stats.purged_transactions, 5);
        assert_eq!(stats.purged_logs, 5);
        assert_eq!(Block::count(&conn).await?, 0);

        Ok(())
    }

    #[tokio::test]
    async fn purge_on_empty_db_is_a_noop() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let stats = purge_old_blocks(10, &conn).await?;
        assert_eq!(stats, PurgeStats::default());

        Ok(())
    }
}
