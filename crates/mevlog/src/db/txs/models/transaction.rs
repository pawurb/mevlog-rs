use std::collections::HashSet;

use std::str::FromStr;

use alloy::rlp::Encodable;
use arrow::record_batch::RecordBatch;
use eyre::Result;
use revm::primitives::{Address, Bytes, FixedBytes, TxKind, U256, keccak256};
use sqlx::{Row, SqlitePool, sqlite::SqliteRow};

use crate::{db::sigs::models::method::Method, misc::parquet_utils::get_parquet_string_value};

const UNKNOWN_SIGNATURE: &str = "?";

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

#[hotpath::measure_all(future = true)]
impl Transaction {
    // Parquet columns: 0 block_number, 1 transaction_index, 2 transaction_hash,
    // 3 nonce, 4 from_address, 5 to_address, 7 value_string, 9 input,
    // 10 gas_limit, 11 gas_used, 12 gas_price, 13 transaction_type,
    // 14 max_priority_fee_per_gas, 15 max_fee_per_gas, 16 success.
    pub async fn from_parquet_row(
        batch: &RecordBatch,
        row_idx: usize,
        sqlite: &SqlitePool,
    ) -> Result<(Transaction, u64)> {
        let get = |col_idx: usize| -> String { get_parquet_string_value(batch, col_idx, row_idx) };

        let block_number = get(0).parse::<u64>().unwrap();
        let tx_index = get(1).parse::<u64>().unwrap();
        let tx_hash = FixedBytes::<32>::from_str(&get(2)).unwrap();
        let nonce = get(3).parse::<u64>().unwrap();
        let from_address = Address::from_str(&get(4)).unwrap();

        let to_str = get(5);
        let to = if to_str == "0x" || to_str.is_empty() {
            TxKind::Create
        } else {
            TxKind::Call(Address::from_str(&to_str).unwrap())
        };

        let input = Bytes::from_str(&get(9)).unwrap();
        let (signature_hash, signature) =
            extract_signature(Some(&input), tx_index, Some(to), sqlite).await?;

        let to_address = match to {
            TxKind::Call(address) => Some(address),
            TxKind::Create => Some(calculate_create_address(nonce, from_address)),
        };

        let gas_price = get(12).parse::<u128>().unwrap();

        let tx = Transaction {
            block_number,
            tx_index,
            tx_hash,
            nonce,
            from_address,
            to_address,
            value: U256::from_str(&get(7)).unwrap(),
            gas_limit: get(10).parse::<u64>().unwrap(),
            gas_used: get(11).parse::<u64>().unwrap(),
            effective_gas_price: gas_price,
            gas_price,
            max_fee_per_gas: get(15).parse::<u128>().unwrap_or(0),
            max_priority_fee_per_gas: get(14).parse::<u128>().unwrap_or(0),
            transaction_type: get(13).parse::<u8>().ok(),
            success: get(16).parse::<bool>().unwrap(),
            signature_hash,
            signature,
        };

        Ok((tx, block_number))
    }

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
        // SQLite INTEGER is signed 64-bit. Gas prices are modeled as u128, so a
        // value above i64::MAX would wrap when stored and read back corrupted.
        // Skip such txs rather than persist a bogus gas price.
        let (
            Ok(effective_gas_price),
            Ok(gas_price),
            Ok(max_fee_per_gas),
            Ok(max_priority_fee_per_gas),
        ) = (
            i64::try_from(self.effective_gas_price),
            i64::try_from(self.gas_price),
            i64::try_from(self.max_fee_per_gas),
            i64::try_from(self.max_priority_fee_per_gas),
        )
        else {
            tracing::warn!(
                "Skipping tx 0x{}: gas price exceeds i64::MAX, cannot store",
                hex::encode(self.tx_hash)
            );
            return Ok(());
        };

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
        .bind(self.value.to_be_bytes::<32>().to_vec())
        .bind(self.gas_limit as i64)
        .bind(self.gas_used as i64)
        .bind(effective_gas_price)
        .bind(gas_price)
        .bind(max_fee_per_gas)
        .bind(max_priority_fee_per_gas)
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

        let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
            .fetch_all(conn)
            .await?;
        rows.iter().map(Self::from_row).collect()
    }

    fn from_row(row: &SqliteRow) -> Result<Transaction> {
        let block_number: i64 = row.try_get("block_number")?;
        let tx_index: i64 = row.try_get("tx_index")?;
        let tx_hash: Vec<u8> = row.try_get("tx_hash")?;
        let nonce: i64 = row.try_get("nonce")?;
        let from_address: Vec<u8> = row.try_get("from_address")?;
        let to_address: Option<Vec<u8>> = row.try_get("to_address")?;
        let value: Vec<u8> = row.try_get("value")?;
        let gas_limit: i64 = row.try_get("gas_limit")?;
        let gas_used: i64 = row.try_get("gas_used")?;
        let effective_gas_price: i64 = row.try_get("effective_gas_price")?;
        let gas_price: i64 = row.try_get("gas_price")?;
        let max_fee_per_gas: i64 = row.try_get("max_fee_per_gas")?;
        let max_priority_fee_per_gas: i64 = row.try_get("max_priority_fee_per_gas")?;
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
            value: U256::from_be_slice(&value),
            gas_limit: gas_limit as u64,
            gas_used: gas_used as u64,
            effective_gas_price: effective_gas_price as u128,
            gas_price: gas_price as u128,
            max_fee_per_gas: max_fee_per_gas as u128,
            max_priority_fee_per_gas: max_priority_fee_per_gas as u128,
            transaction_type: transaction_type.map(|t| t as u8),
            success,
            signature_hash: signature_hash.map(|b| FixedBytes::<4>::from_slice(&b)),
            signature,
        })
    }
}

pub async fn extract_signature(
    input: Option<&Bytes>,
    index: u64,
    to: Option<TxKind>,
    sqlite: &SqlitePool,
) -> Result<(Option<FixedBytes<4>>, Option<String>)> {
    if to == Some(TxKind::Create) || to.is_none() {
        return Ok((None, Some("CREATE()".to_string())));
    }

    let signature_hash = input
        .filter(|i| i.len() >= 4)
        .map(|i| FixedBytes::<4>::from_slice(&i[..4]));

    let signature = match signature_hash {
        Some(hash) => {
            let sig = format!("0x{}", hex::encode(hash));
            let resolved = if let Some(sig_overwrite) = find_sig_overwrite(&sig, index) {
                Some(sig_overwrite)
            } else {
                Method::find_by_selector(&sig, sqlite).await?
            };
            Some(resolved.unwrap_or_else(|| UNKNOWN_SIGNATURE.to_string()))
        }
        // No calldata: plain ETH transfer
        None => None,
    };

    Ok((signature_hash, signature))
}

pub fn calculate_create_address(nonce: u64, from: Address) -> Address {
    let mut out = Vec::new();
    let list: [&dyn Encodable; 2] = [&from, &U256::from(nonce)];
    alloy::rlp::encode_list::<_, dyn Encodable>(&list, &mut out);
    let keccak = keccak256(&out);
    Address::from_slice(&keccak[12..])
}

// Common signatures, that are duplicate and mismatched in the database
pub fn find_sig_overwrite(signature: &str, tx_index: u64) -> Option<String> {
    if signature == "0x098999be" && tx_index == 0 {
        return Some("setL1BlockValuesIsthmus()".to_string());
    }
    None
}

#[cfg(test)]
pub mod test {
    use std::fs;

    use sqlx::sqlite::SqlitePool;
    use uuid::Uuid;

    use super::*;
    use crate::db::txs::{conn, init_db};

    pub async fn setup_test_db() -> (SqlitePool, SqliteCleaner) {
        let (write, _path, cleaner) = setup_test_db_rw().await;
        (write, cleaner)
    }

    /// Returns a writable pool plus the on-disk path of the same file. The path
    /// is what the read-only `rusqlite`-backed `run_raw_query` consumes.
    pub async fn setup_test_db_rw() -> (SqlitePool, String, SqliteCleaner) {
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

        let write = conn(Some(db_url.clone()), 1, false)
            .await
            .expect("Failed to connect to db");

        (write, db_path, cleaner)
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
