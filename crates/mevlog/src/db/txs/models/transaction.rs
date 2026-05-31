use std::{collections::HashSet, str::FromStr};

use eyre::Result;
use revm::primitives::{Address, FixedBytes, U256};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

/// Basic SQLite-backed transaction record.
///
/// Holds only the core transaction + receipt fields. Logs/events and EVM traces
/// are intentionally excluded and will be stored in separate tables later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub block_number: u64,
    pub tx_index: u64,
    pub tx_hash: FixedBytes<32>,
    pub nonce: u64,
    pub from_address: Address,
    /// `None` for contract-creation transactions.
    pub to_address: Option<Address>,
    pub value: U256,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub effective_gas_price: u128,
    pub gas_price: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub transaction_type: Option<u8>,
    pub success: bool,
    /// First 4 bytes of the calldata (function selector).
    /// `None` for contract-creation transactions or calldata shorter than 4 bytes.
    pub signature_hash: Option<FixedBytes<4>>,
    /// `None` when the method signature could not be resolved.
    pub signature: Option<String>,
}

/// JSON-serializable view of a [`Transaction`].
///
/// Hashes and addresses are hex-encoded (via alloy's `Serialize`), while the
/// 256-bit `value` is stringified to avoid lossy JSON number handling.
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TransactionJson {
    pub block_number: u64,
    pub tx_index: u64,
    pub tx_hash: FixedBytes<32>,
    pub nonce: u64,
    pub from: Address,
    pub to: Option<Address>,
    pub value: String,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub effective_gas_price: u128,
    pub gas_price: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub transaction_type: Option<u8>,
    pub success: bool,
    pub signature_hash: Option<String>,
    pub signature: Option<String>,
}

impl From<&Transaction> for TransactionJson {
    fn from(tx: &Transaction) -> Self {
        Self {
            block_number: tx.block_number,
            tx_index: tx.tx_index,
            tx_hash: tx.tx_hash,
            nonce: tx.nonce,
            from: tx.from_address,
            to: tx.to_address,
            value: tx.value.to_string(),
            gas_limit: tx.gas_limit,
            gas_used: tx.gas_used,
            effective_gas_price: tx.effective_gas_price,
            gas_price: tx.gas_price,
            max_fee_per_gas: tx.max_fee_per_gas,
            max_priority_fee_per_gas: tx.max_priority_fee_per_gas,
            transaction_type: tx.transaction_type,
            success: tx.success,
            signature_hash: tx.signature_hash.map(|s| format!("0x{}", hex::encode(s))),
            signature: tx.signature.clone(),
        }
    }
}

#[hotpath::measure_all(future = true)]
impl Transaction {
    pub async fn count(conn: &SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM transactions")
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
            INSERT INTO transactions (
                block_number, tx_index, tx_hash, nonce, from_address, to_address,
                value, gas_limit, gas_used, effective_gas_price, gas_price,
                max_fee_per_gas, max_priority_fee_per_gas, transaction_type,
                success, signature_hash, signature
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(tx_hash) DO NOTHING
            "#,
        )
        .bind(self.block_number as i64)
        .bind(self.tx_index as i64)
        .bind(self.tx_hash.as_slice())
        .bind(self.nonce as i64)
        .bind(self.from_address.as_slice())
        .bind(self.to_address.as_ref().map(|a| a.as_slice()))
        .bind(self.value.to_string())
        .bind(self.gas_limit as i64)
        .bind(self.gas_used as i64)
        .bind(self.effective_gas_price.to_string())
        .bind(self.gas_price.to_string())
        .bind(self.max_fee_per_gas.to_string())
        .bind(self.max_priority_fee_per_gas.to_string())
        .bind(self.transaction_type.map(|t| t as i64))
        .bind(self.success)
        .bind(self.signature_hash.as_ref().map(|s| s.as_slice()))
        .bind(self.signature.as_deref())
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn mark_indexed<'c, E>(block_number: u64, executor: E) -> Result<()>
    where
        E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
    {
        sqlx::query(
            "INSERT INTO indexed_blocks (block_number) VALUES (?) \
             ON CONFLICT(block_number) DO NOTHING",
        )
        .bind(block_number as i64)
        .execute(executor)
        .await?;

        Ok(())
    }

    pub async fn save_batch(txs: &[Transaction], blocks: &[u64], conn: &SqlitePool) -> Result<()> {
        let mut db_tx = conn.begin().await?;

        for tx in txs {
            tx.save(&mut *db_tx).await?;
        }
        for &block in blocks {
            Self::mark_indexed(block, &mut *db_tx).await?;
        }

        db_tx.commit().await?;
        Ok(())
    }

    pub async fn missing_blocks(from: u64, to: u64, conn: &SqlitePool) -> Result<Vec<u64>> {
        let existing: Vec<i64> = sqlx::query_scalar(
            "SELECT block_number FROM indexed_blocks WHERE block_number BETWEEN ? AND ?",
        )
        .bind(from as i64)
        .bind(to as i64)
        .fetch_all(conn)
        .await?;

        let indexed: HashSet<u64> = existing.into_iter().map(|b| b as u64).collect();
        Ok((from..=to).filter(|b| !indexed.contains(b)).collect())
    }

    pub async fn query_where(where_sql: &str, conn: &SqlitePool) -> Result<Vec<Transaction>> {
        let sql = format!(
            "SELECT * FROM transactions WHERE {where_sql} \
             ORDER BY block_number DESC, tx_index ASC"
        );

        let rows = sqlx::query(sqlx::AssertSqlSafe(sql)).fetch_all(conn).await?;
        rows.iter().map(Self::from_row).collect()
    }

    fn from_row(row: &SqliteRow) -> Result<Transaction> {
        let block_number: i64 = row.try_get("block_number")?;
        let tx_index: i64 = row.try_get("tx_index")?;
        let tx_hash: Vec<u8> = row.try_get("tx_hash")?;
        let nonce: i64 = row.try_get("nonce")?;
        let from_address: Vec<u8> = row.try_get("from_address")?;
        let to_address: Option<Vec<u8>> = row.try_get("to_address")?;
        let value: String = row.try_get("value")?;
        let gas_limit: i64 = row.try_get("gas_limit")?;
        let gas_used: i64 = row.try_get("gas_used")?;
        let effective_gas_price: String = row.try_get("effective_gas_price")?;
        let gas_price: String = row.try_get("gas_price")?;
        let max_fee_per_gas: String = row.try_get("max_fee_per_gas")?;
        let max_priority_fee_per_gas: String = row.try_get("max_priority_fee_per_gas")?;
        let transaction_type: Option<i64> = row.try_get("transaction_type")?;
        let success: bool = row.try_get("success")?;
        let signature_hash: Option<Vec<u8>> = row.try_get("signature_hash")?;
        let signature: Option<String> = row.try_get("signature")?;

        Ok(Transaction {
            block_number: block_number as u64,
            tx_index: tx_index as u64,
            tx_hash: FixedBytes::<32>::from_slice(&tx_hash),
            nonce: nonce as u64,
            from_address: Address::from_slice(&from_address),
            to_address: to_address.map(|b| Address::from_slice(&b)),
            value: U256::from_str(&value)?,
            gas_limit: gas_limit as u64,
            gas_used: gas_used as u64,
            effective_gas_price: effective_gas_price.parse()?,
            gas_price: gas_price.parse()?,
            max_fee_per_gas: max_fee_per_gas.parse()?,
            max_priority_fee_per_gas: max_priority_fee_per_gas.parse()?,
            transaction_type: transaction_type.map(|t| t as u8),
            success,
            signature_hash: signature_hash.map(|b| FixedBytes::<4>::from_slice(&b)),
            signature,
        })
    }
}

#[cfg(test)]
pub mod test {
    use std::fs;

    use sqlx::sqlite::SqlitePool;
    use uuid::Uuid;

    use super::*;
    use crate::db::txs::{conn, init_db};

    pub async fn setup_test_db() -> (SqlitePool, SqliteCleaner) {
        let uuid = Uuid::new_v4();
        let db_path = format!("/tmp/{uuid}-mevlog-txs-test.db");
        let db_url = format!("sqlite://{db_path}");

        if fs::remove_file(&db_url).is_ok() {
            println!("DB {} removed", &db_url);
        }

        // `db_url` overrides the per-chain path, so the chain_id is irrelevant here.
        init_db(Some(db_url.clone()), 1)
            .await
            .expect("Failed to init db");

        let cleaner = SqliteCleaner {
            db_uuid: uuid.to_string(),
        };

        (
            conn(Some(db_url), 1)
                .await
                .expect("Failed to connect to db"),
            cleaner,
        )
    }

    pub struct SqliteCleaner {
        pub db_uuid: String,
    }

    impl Drop for SqliteCleaner {
        fn drop(&mut self) {
            let pattern = format!("/tmp/*{}*", self.db_uuid);

            for entry in glob::glob(&pattern).expect("Failed to read glob pattern") {
                match entry {
                    Ok(path) => {
                        if let Err(e) = fs::remove_file(&path) {
                            eprintln!("Failed to remove file {path:?}: {e}");
                        }
                    }
                    Err(e) => eprintln!("Error reading glob entry: {e}"),
                }
            }
        }
    }

    fn sample_tx(block_number: u64, tx_index: u64, hash_byte: u8) -> Transaction {
        Transaction {
            block_number,
            tx_index,
            tx_hash: FixedBytes::<32>::from([hash_byte; 32]),
            nonce: 7,
            from_address: Address::from([0x11; 20]),
            to_address: Some(Address::from([0x22; 20])),
            value: U256::from(1_000_000_000_000_000_000u128),
            gas_limit: 21_000,
            gas_used: 21_000,
            effective_gas_price: 30_000_000_000,
            gas_price: 30_000_000_000,
            max_fee_per_gas: 40_000_000_000,
            max_priority_fee_per_gas: 2_000_000_000,
            transaction_type: Some(2),
            success: true,
            signature_hash: Some(FixedBytes::<4>::from([0xa9, 0x05, 0x9c, 0xbb])),
            signature: Some("transfer(address,uint256)".to_string()),
        }
    }

    #[tokio::test]
    async fn save_batch_and_query_roundtrips() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let tx = sample_tx(100, 0, 0xaa);
        Transaction::save_batch(std::slice::from_ref(&tx), &[100], &conn).await?;

        assert_eq!(Transaction::count(&conn).await?, 1);

        let found = Transaction::query_where("block_number = 100", &conn).await?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0], tx);

        Ok(())
    }

    #[tokio::test]
    async fn save_batch_is_idempotent() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let tx = sample_tx(100, 0, 0xaa);
        Transaction::save_batch(std::slice::from_ref(&tx), &[100], &conn).await?;
        Transaction::save_batch(std::slice::from_ref(&tx), &[100], &conn).await?;

        assert_eq!(Transaction::count(&conn).await?, 1);
        Ok(())
    }

    #[test]
    fn transaction_json_encodes_hex_and_stringifies_value() {
        let tx = sample_tx(100, 3, 0xaa);
        let json = TransactionJson::from(&tx);
        let value = serde_json::to_value(&json).expect("serialize");

        assert_eq!(value["block_number"], 100);
        assert_eq!(value["tx_index"], 3);
        assert_eq!(value["tx_hash"], format!("0x{}", "aa".repeat(32)));
        assert_eq!(value["from"], format!("0x{}", "11".repeat(20)));
        assert_eq!(value["to"], format!("0x{}", "22".repeat(20)));
        // U256 value serialized as a string, not a JSON number.
        assert_eq!(value["value"], "1000000000000000000");
        assert_eq!(value["signature_hash"], "0xa9059cbb");
        assert_eq!(value["signature"], "transfer(address,uint256)");
    }

    #[tokio::test]
    async fn missing_blocks_excludes_indexed() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        // Block 101 has no txs but is still marked indexed.
        Transaction::save_batch(&[], &[101], &conn).await?;
        Transaction::save_batch(&[sample_tx(103, 0, 0xbb)], &[103], &conn).await?;

        let missing = Transaction::missing_blocks(100, 104, &conn).await?;
        assert_eq!(missing, vec![100, 102, 104]);

        Ok(())
    }
}
