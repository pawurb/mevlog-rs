use eyre::Result;
use revm::primitives::{Address, FixedBytes, U256};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use crate::{misc::utils::UNKNOWN, models::mev_log::MEVLog};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Log {
    pub block_number: u64,
    pub tx_index: u64,
    pub log_index: u64,
    /// Emitting contract address.
    pub address: Address,
    /// Indexed topics (`topic0` is the event signature hash). At most 4.
    pub topics: Vec<FixedBytes<32>>,
    pub data: Vec<u8>,
    /// Decoded ERC20 transfer value. `None` for non-transfer logs.
    pub erc20_amount: Option<U256>,
    /// `None` when the event signature could not be resolved.
    pub signature: Option<String>,
}

impl Log {
    /// Builds a [`Log`] from an already-parsed [`MEVLog`] and its block number.
    ///
    /// `erc20_amount` is carried over (already decoded for ERC20 transfers).
    pub fn from_mev_log(block_number: u64, log: &MEVLog) -> Log {
        let signature =
            (log.signature.signature != UNKNOWN).then(|| log.signature.signature.clone());

        Log {
            block_number,
            tx_index: log.tx_index,
            log_index: log.log_index,
            address: log.source,
            topics: log.topics.clone(),
            data: log.data.clone(),
            erc20_amount: log.signature.amount,
            signature,
        }
    }
}

#[hotpath::measure_all(future = true)]
impl Log {
    pub async fn count(conn: &SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM logs")
            .fetch_one(conn)
            .await?
            .get::<i64, _>(0);

        Ok(count)
    }

    pub async fn save<'c, E>(&self, executor: E) -> Result<()>
    where
        E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
    {
        let topic = |i: usize| self.topics.get(i).map(|t| t.as_slice());

        sqlx::query(
            r#"
            INSERT INTO logs (
                block_number, tx_index, log_index, address,
                topic0, topic1, topic2, topic3, data, erc20_amount, signature
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(block_number, log_index) DO NOTHING
            "#,
        )
        .bind(self.block_number as i64)
        .bind(self.tx_index as i64)
        .bind(self.log_index as i64)
        .bind(self.address.as_slice())
        .bind(topic(0))
        .bind(topic(1))
        .bind(topic(2))
        .bind(topic(3))
        .bind(self.data.as_slice())
        .bind(self.erc20_amount.map(|a| a.to_be_bytes::<32>().to_vec()))
        .bind(self.signature.as_deref())
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn save_batch(logs: &[Log], conn: &SqlitePool) -> Result<()> {
        let mut db_tx = conn.begin().await?;

        for log in logs {
            log.save(&mut *db_tx).await?;
        }

        db_tx.commit().await?;
        Ok(())
    }

    pub async fn query_where(where_sql: &str, conn: &SqlitePool) -> Result<Vec<Log>> {
        let sql = format!(
            "SELECT * FROM logs WHERE {where_sql} \
             ORDER BY block_number DESC, log_index ASC"
        );

        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(conn)
            .await?;
        rows.iter().map(Self::from_row).collect()
    }

    fn from_row(row: &SqliteRow) -> Result<Log> {
        let block_number: i64 = row.try_get("block_number")?;
        let tx_index: i64 = row.try_get("tx_index")?;
        let log_index: i64 = row.try_get("log_index")?;
        let address: Vec<u8> = row.try_get("address")?;
        let data: Vec<u8> = row.try_get("data")?;
        let erc20_amount: Option<Vec<u8>> = row.try_get("erc20_amount")?;
        let signature: Option<String> = row.try_get("signature")?;

        let mut topics = Vec::new();
        for col in ["topic0", "topic1", "topic2", "topic3"] {
            let topic: Option<Vec<u8>> = row.try_get(col)?;
            match topic {
                Some(bytes) => topics.push(FixedBytes::<32>::from_slice(&bytes)),
                None => break,
            }
        }

        Ok(Log {
            block_number: block_number as u64,
            tx_index: tx_index as u64,
            log_index: log_index as u64,
            address: Address::from_slice(&address),
            topics,
            data,
            erc20_amount: erc20_amount.map(|b| U256::from_be_slice(&b)),
            signature,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::db::txs::models::transaction::test::setup_test_db;

    fn sample_log(block_number: u64, log_index: u64, erc20_amount: Option<U256>) -> Log {
        Log {
            block_number,
            tx_index: 0,
            log_index,
            address: Address::from([0x11; 20]),
            topics: vec![
                FixedBytes::<32>::from([0xdd; 32]),
                FixedBytes::<32>::from([0x01; 32]),
                FixedBytes::<32>::from([0x02; 32]),
            ],
            data: vec![0xde, 0xad, 0xbe, 0xef],
            erc20_amount,
            signature: Some("Transfer(address,address,uint256)".to_string()),
        }
    }

    #[tokio::test]
    async fn save_batch_and_query_roundtrips() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let log = sample_log(100, 0, Some(U256::from(1_000_000_000u64)));
        Log::save_batch(std::slice::from_ref(&log), &conn).await?;

        assert_eq!(Log::count(&conn).await?, 1);

        let found = Log::query_where("block_number = 100", &conn).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], log);

        Ok(())
    }

    #[tokio::test]
    async fn save_batch_is_idempotent() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let log = sample_log(100, 0, None);
        Log::save_batch(std::slice::from_ref(&log), &conn).await?;
        Log::save_batch(std::slice::from_ref(&log), &conn).await?;

        assert_eq!(Log::count(&conn).await?, 1);
        Ok(())
    }

    #[tokio::test]
    async fn erc20_amount_blob_compares_numerically() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let small = sample_log(100, 0, Some(U256::from(500u64)));
        let large = sample_log(100, 1, Some(U256::from(2_000u64)));
        Log::save_batch(&[small, large], &conn).await?;

        // 1000 left-padded to 32 bytes (big-endian) -> lexicographic blob compare.
        let threshold = hex::encode(U256::from(1_000u64).to_be_bytes::<32>());
        let found = Log::query_where(&format!("erc20_amount > X'{threshold}'"), &conn).await?;

        assert_eq!(found.len(), 1);
        assert_eq!(found[0].erc20_amount, Some(U256::from(2_000u64)));

        Ok(())
    }
}
