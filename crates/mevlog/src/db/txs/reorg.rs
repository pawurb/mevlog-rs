use std::future::Future;

use eyre::Result;
use revm::primitives::FixedBytes;
use sqlx::SqlitePool;

use crate::db::txs::{custom_tables, models::block::Block};

/// Row counts removed by [`rollback_from`]. Custom-table rows are deleted too
/// but not counted, mirroring [`crate::db::txs::purge::PurgeStats`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RollbackStats {
    pub blocks: u64,
    pub transactions: u64,
    pub logs: u64,
}

/// Finds the highest locally indexed block at or below `tip` that is still on
/// the canonical chain, by comparing stored block hashes against
/// `remote_hash(block_number)` (the canonical hash, or `None` when the chain
/// no longer has that block), walking backwards from `tip`.
///
/// Returns `Ok(None)` when there is no reorg: the stored hash at `tip` matches
/// the canonical one, or the local store has no row at `tip` to compare.
/// Returns `Ok(Some(fork_point))` when hashes diverge; every stored block
/// above `fork_point` is stale and must be rolled back. Errors when no match
/// is found within `max_depth` blocks (or within the contiguous run of stored
/// blocks below `tip`) - the reorg is deeper than the scan window and the
/// store needs a manual purge.
pub async fn find_fork_point<F, Fut>(
    tip: u64,
    max_depth: u64,
    conn: &SqlitePool,
    remote_hash: F,
) -> Result<Option<u64>>
where
    F: Fn(u64) -> Fut,
    Fut: Future<Output = Result<Option<FixedBytes<32>>>>,
{
    let stored = Block::tip_hashes(tip, max_depth, conn).await?;

    // Nothing indexed at `tip` (empty store, or a gap right below it): there
    // is no local hash to contradict the canonical chain.
    match stored.first() {
        Some((number, _)) if *number == tip => {}
        _ => return Ok(None),
    }

    for (i, (number, local_hash)) in stored.iter().enumerate() {
        if *number != tip - i as u64 {
            // Gap in the stored run: blocks below it cannot extend the
            // verification chain, so treat like an exhausted scan window.
            break;
        }

        if remote_hash(*number).await? == Some(*local_hash) {
            return if i == 0 { Ok(None) } else { Ok(Some(*number)) };
        }
    }

    eyre::bail!(
        "Reorg deeper than {} block(s) below block {}; purge the local store (e.g. 'mevlog purge-db') and reindex",
        max_depth,
        tip
    )
}

/// Returns the lowest block in `[from, to]` whose stored `parent_hash` does
/// not match the stored hash of its predecessor, or `None` when every
/// verifiable link holds. Rows with a `NULL` `parent_hash` (pre-v2 parquet
/// cache) and blocks whose predecessor is not indexed are skipped.
pub async fn first_parent_link_break(from: u64, to: u64, conn: &SqlitePool) -> Result<Option<u64>> {
    let broken: Option<i64> = sqlx::query_scalar(
        "SELECT MIN(b.block_number) FROM blocks b
         JOIN blocks p ON p.block_number = b.block_number - 1
         WHERE b.block_number BETWEEN ? AND ?
           AND b.parent_hash IS NOT NULL
           AND b.parent_hash != p.block_hash",
    )
    .bind(from as i64)
    .bind(to as i64)
    .fetch_one(conn)
    .await?;

    Ok(broken.map(|b| b as u64))
}

/// Deletes all indexed data for blocks `>= first_stale_block` from `blocks`,
/// `transactions`, `logs`, and every tracked custom table, in one transaction.
/// Reorg depth is bounded by the scan window, so the deletion is small enough
/// to skip the chunking that [`crate::db::txs::purge::purge_old_blocks`] needs.
pub async fn rollback_from(first_stale_block: u64, conn: &SqlitePool) -> Result<RollbackStats> {
    let custom_tables = custom_tables::tracked_table_names(conn).await?;

    let mut db_tx = conn.begin().await?;
    let mut stats = RollbackStats::default();

    for name in &custom_tables {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DELETE FROM \"{name}\" WHERE block_number >= ?"
        )))
        .bind(first_stale_block as i64)
        .execute(&mut *db_tx)
        .await?;
    }

    stats.logs = sqlx::query("DELETE FROM logs WHERE block_number >= ?")
        .bind(first_stale_block as i64)
        .execute(&mut *db_tx)
        .await?
        .rows_affected();

    stats.transactions = sqlx::query("DELETE FROM transactions WHERE block_number >= ?")
        .bind(first_stale_block as i64)
        .execute(&mut *db_tx)
        .await?
        .rows_affected();

    stats.blocks = sqlx::query("DELETE FROM blocks WHERE block_number >= ?")
        .bind(first_stale_block as i64)
        .execute(&mut *db_tx)
        .await?
        .rows_affected();

    db_tx.commit().await?;
    Ok(stats)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use revm::primitives::{Address, U256};

    use super::*;
    use crate::db::txs::models::{
        log::Log,
        transaction::{Transaction, test::setup_test_db},
    };

    fn hash(byte: u8) -> FixedBytes<32> {
        FixedBytes::<32>::from([byte; 32])
    }

    /// Chained blocks: block N has hash [N as u8; 32] and parent_hash pointing
    /// at block N-1's hash.
    fn chained_block(block_number: u64) -> Block {
        Block {
            block_number,
            block_hash: hash(block_number as u8),
            parent_hash: Some(hash(block_number as u8 - 1)),
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
            topics: vec![hash(0xdd)],
            data: vec![0xde, 0xad],
            erc20_amount: None,
            signature: None,
        }
    }

    async fn seed_chain(range: std::ops::RangeInclusive<u64>, conn: &SqlitePool) -> Result<()> {
        let blocks: Vec<Block> = range.clone().map(chained_block).collect();
        let txs: Vec<Transaction> = range.clone().map(sample_tx).collect();
        let logs: Vec<Log> = range.map(sample_log).collect();

        Block::save_batch(&blocks, conn).await?;
        Transaction::save_batch(&txs, conn).await?;
        Log::save_batch(&logs, conn).await?;
        Ok(())
    }

    fn remote(canonical: HashMap<u64, FixedBytes<32>>) -> impl Fn(u64) -> RemoteFut {
        move |number| {
            let hash = canonical.get(&number).copied();
            Box::pin(async move { Ok(hash) })
        }
    }

    type RemoteFut = std::pin::Pin<Box<dyn Future<Output = Result<Option<FixedBytes<32>>>> + Send>>;

    #[tokio::test]
    async fn no_reorg_when_tip_hash_matches() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_chain(100..=104, &conn).await?;

        let canonical: HashMap<u64, FixedBytes<32>> =
            (100..=104).map(|n| (n, hash(n as u8))).collect();

        let fork = find_fork_point(104, 64, &conn, remote(canonical)).await?;
        assert_eq!(fork, None);
        Ok(())
    }

    #[tokio::test]
    async fn finds_fork_point_below_reorged_blocks() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_chain(100..=104, &conn).await?;

        // Chain reorged: blocks 103 and 104 replaced.
        let canonical: HashMap<u64, FixedBytes<32>> = (100..=104)
            .map(|n| (n, if n >= 103 { hash(0xff) } else { hash(n as u8) }))
            .collect();

        let fork = find_fork_point(104, 64, &conn, remote(canonical)).await?;
        assert_eq!(fork, Some(102));
        Ok(())
    }

    #[tokio::test]
    async fn shortened_chain_counts_as_mismatch() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_chain(100..=104, &conn).await?;

        // Canonical chain now ends at 102 (remote_hash yields None above it).
        let canonical: HashMap<u64, FixedBytes<32>> =
            (100..=102).map(|n| (n, hash(n as u8))).collect();

        let fork = find_fork_point(104, 64, &conn, remote(canonical)).await?;
        assert_eq!(fork, Some(102));
        Ok(())
    }

    #[tokio::test]
    async fn no_check_without_stored_tip() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_chain(100..=102, &conn).await?;

        // No stored row at 105: nothing to verify.
        let fork = find_fork_point(105, 64, &conn, remote(HashMap::new())).await?;
        assert_eq!(fork, None);
        Ok(())
    }

    #[tokio::test]
    async fn errors_when_reorg_deeper_than_scan_window() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_chain(100..=104, &conn).await?;

        // Every canonical hash differs from the stored one.
        let canonical: HashMap<u64, FixedBytes<32>> =
            (100..=104).map(|n| (n, hash(0xff))).collect();

        let result = find_fork_point(104, 3, &conn, remote(canonical)).await;
        assert!(result.is_err());
        Ok(())
    }

    #[tokio::test]
    async fn parent_link_break_found() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut blocks: Vec<Block> = (100..=104).map(chained_block).collect();
        // Block 103 points at a parent that is not block 102's hash.
        blocks[3].parent_hash = Some(hash(0xff));
        Block::save_batch(&blocks, &conn).await?;

        assert_eq!(first_parent_link_break(100, 104, &conn).await?, Some(103));
        Ok(())
    }

    #[tokio::test]
    async fn parent_link_intact_or_unverifiable() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut blocks: Vec<Block> = (100..=104).map(chained_block).collect();
        // NULL parent_hash rows (pre-v2 parquet cache) are skipped.
        blocks[2].parent_hash = None;
        Block::save_batch(&blocks, &conn).await?;

        assert_eq!(first_parent_link_break(100, 104, &conn).await?, None);
        Ok(())
    }

    #[tokio::test]
    async fn rollback_removes_stale_rows() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;
        seed_chain(100..=104, &conn).await?;

        let stats = rollback_from(103, &conn).await?;
        assert_eq!(
            stats,
            RollbackStats {
                blocks: 2,
                transactions: 2,
                logs: 2,
            }
        );

        let remaining: Vec<u64> = Block::query_where("1 = 1", &conn)
            .await?
            .iter()
            .map(|b| b.block_number)
            .collect();
        assert_eq!(remaining, vec![102, 101, 100]);

        assert!(
            Transaction::query_where("block_number >= 103", &conn)
                .await?
                .is_empty()
        );
        assert!(
            Log::query_where("block_number >= 103", &conn)
                .await?
                .is_empty()
        );

        Ok(())
    }
}
