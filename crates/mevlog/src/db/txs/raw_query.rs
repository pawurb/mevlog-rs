use std::collections::HashSet;

use evm_sqlite::register_functions;
use eyre::{Result, bail};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use serde_json::{Map, Value};

/// Result of a raw SQL query: the selected column names (in `SELECT` order) plus
/// one JSON object per row. Columns are carried separately so tabular consumers
/// can render headers even when no rows are returned.
#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

/// Runs a user-provided SQL statement against the read-only txs DB and
/// serializes each result row into a JSON object keyed by column name.
///
/// Uses a read-only `rusqlite` connection rather than `sqlx` so the custom
/// `u256_sum` SQL function is available to the query.
pub fn run_raw_query(sql: &str, db_path: &str) -> Result<QueryResult> {
    // Accept both `sqlite://<path>` URLs and bare filesystem paths.
    let filename = db_path
        .strip_prefix("sqlite://")
        .or_else(|| db_path.strip_prefix("sqlite:"))
        .unwrap_or(db_path);

    let conn = Connection::open_with_flags(
        filename,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )?;
    register_functions(&conn)?;

    let mut stmt = conn.prepare(sql)?;
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();

    // Rows are keyed by column name, so duplicate names would silently collapse
    // (last value wins). Reject them instead of emitting corrupt output.
    let mut seen = HashSet::with_capacity(columns.len());
    if let Some(dup) = columns.iter().find(|c| !seen.insert(c.as_str())) {
        bail!("query returns duplicate column name `{dup}`; alias columns to make them unique");
    }

    let col_count = columns.len();
    let mut out = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let mut obj = Map::with_capacity(col_count);
        for (i, col) in columns.iter().enumerate() {
            let value = match row.get_ref(i)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(n) => Value::from(n),
                ValueRef::Real(f) => Value::from(f),
                ValueRef::Text(t) => Value::from(String::from_utf8_lossy(t).into_owned()),
                // BLOB (addresses, hashes, u256 BLOBs): emit raw bytes as 0x-hex.
                ValueRef::Blob(b) => Value::from(format!("0x{}", hex::encode(b))),
            };
            obj.insert(col.clone(), value);
        }
        out.push(Value::Object(obj));
    }

    Ok(QueryResult { columns, rows: out })
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
            coinbase_transfer: None,
        }
    }

    #[tokio::test]
    async fn raw_query_encodes_blobs_as_hex_and_ints_as_numbers() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let result = run_raw_query(
            "SELECT block_number, tx_hash, from_address, signature FROM transactions",
            &path,
        )?;

        assert_eq!(
            result.columns,
            ["block_number", "tx_hash", "from_address", "signature"]
        );
        assert_eq!(result.rows.len(), 1);
        let row = &result.rows[0];
        assert_eq!(row["block_number"], json!(100));
        assert_eq!(row["tx_hash"], json!(format!("0x{}", "aa".repeat(32))));
        assert_eq!(row["from_address"], json!(format!("0x{}", "11".repeat(20))));
        assert_eq!(row["signature"], json!("transfer(address,uint256)"));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_returns_columns_when_no_rows_match() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let result = run_raw_query(
            "SELECT block_number, tx_hash FROM transactions WHERE 1 = 0",
            &path,
        )?;

        assert!(result.rows.is_empty());
        assert_eq!(result.columns, ["block_number", "tx_hash"]);

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_rejects_duplicate_column_names() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let err = run_raw_query("SELECT 1 AS x, 2 AS x FROM transactions", &path).unwrap_err();
        assert!(err.to_string().contains("duplicate column name `x`"));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_supports_projection_and_aggregates() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let result = run_raw_query("SELECT COUNT(*) AS n FROM transactions", &path)?;
        assert_eq!(result.rows[0]["n"], json!(1));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_rejects_mutating_statements() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        for stmt in [
            "DELETE FROM transactions",
            "UPDATE transactions SET nonce = 0",
            "DROP TABLE transactions",
        ] {
            let err = run_raw_query(stmt, &path);
            assert!(err.is_err(), "expected `{stmt}` to be rejected");
        }

        let result = run_raw_query("SELECT COUNT(*) AS n FROM transactions", &path)?;
        assert_eq!(result.rows[0]["n"], json!(1));

        Ok(())
    }
}
