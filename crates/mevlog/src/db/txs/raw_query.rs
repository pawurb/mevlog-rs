use eyre::Result;
use serde_json::{Map, Value};
use sqlx::{Column, Row, SqlitePool, TypeInfo, ValueRef};

/// Runs a user-provided SQL statement and serializes each result row into a
/// JSON object keyed by column name. `pool` must be read-only (`txs_read`).
pub async fn run_raw_query(sql: &str, pool: &SqlitePool) -> Result<Vec<Value>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql.to_string()))
        .fetch_all(pool)
        .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut obj = Map::with_capacity(row.columns().len());
        for (i, col) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(i)?;
            let value = if raw.is_null() {
                Value::Null
            } else {
                match raw.type_info().name() {
                    "INTEGER" => Value::from(row.try_get::<i64, _>(i)?),
                    "REAL" => Value::from(row.try_get::<f64, _>(i)?),
                    "TEXT" => Value::from(row.try_get::<String, _>(i)?),
                    // BLOB and anything else: emit raw bytes as 0x-hex.
                    _ => {
                        let bytes: Vec<u8> = row.try_get(i)?;
                        Value::from(format!("0x{}", hex::encode(bytes)))
                    }
                }
            };
            obj.insert(col.name().to_string(), value);
        }
        out.push(Value::Object(obj));
    }

    Ok(out)
}

#[cfg(test)]
mod test {
    use revm::primitives::{Address, FixedBytes, U256};
    use serde_json::json;

    use super::*;
    use crate::db::txs::models::transaction::{Transaction, test::setup_test_db_rw};

    fn sample_tx() -> Transaction {
        Transaction {
            block_number: 100,
            tx_index: 0,
            tx_hash: FixedBytes::<32>::from([0xaa; 32]),
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
    async fn raw_query_encodes_blobs_as_hex_and_ints_as_numbers() -> Result<()> {
        let (write, read, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &[100], &write).await?;

        let rows = run_raw_query(
            "SELECT block_number, tx_hash, from_address, signature FROM transactions",
            &read,
        )
        .await?;

        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["block_number"], json!(100));
        assert_eq!(row["tx_hash"], json!(format!("0x{}", "aa".repeat(32))));
        assert_eq!(row["from_address"], json!(format!("0x{}", "11".repeat(20))));
        assert_eq!(row["signature"], json!("transfer(address,uint256)"));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_supports_projection_and_aggregates() -> Result<()> {
        let (write, read, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &[100], &write).await?;

        let rows = run_raw_query("SELECT COUNT(*) AS n FROM transactions", &read).await?;
        assert_eq!(rows[0]["n"], json!(1));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_rejects_mutating_statements() -> Result<()> {
        let (write, read, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &[100], &write).await?;

        for stmt in [
            "DELETE FROM transactions",
            "UPDATE transactions SET nonce = 0",
            "DROP TABLE transactions",
        ] {
            let err = run_raw_query(stmt, &read).await;
            assert!(err.is_err(), "expected `{stmt}` to be rejected");
        }

        let rows = run_raw_query("SELECT COUNT(*) AS n FROM transactions", &read).await?;
        assert_eq!(rows[0]["n"], json!(1));

        Transaction::save_batch(&[sample_tx()], &[101], &write).await?;

        Ok(())
    }
}
