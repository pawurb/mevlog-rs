use std::{
    collections::HashSet,
    time::{Duration, Instant},
};

use evm_sqlite::register_functions;
use eyre::{Result, bail, eyre};
use rusqlite::{
    Connection, OpenFlags,
    hooks::{AuthAction, AuthContext, Authorization},
    types::ValueRef,
};
use serde_json::{Map, Value};

/// Tables a user-supplied `--sql` query is allowed to read. Everything else
/// (the `_sqlx_migrations` bookkeeping table, attached databases, etc.) is
/// rejected by the authorizer below.
const ALLOWED_TABLES: [&str; 3] = ["transactions", "logs", "blocks"];

/// Read-only PRAGMA table-valued functions a query may call. These expose only
/// the database file's size (`page_count * page_size`), never row data, so they
/// are safe to surface for DB-stats queries even though every real `PRAGMA`
/// statement stays denied by the authorizer. Each `pragma_X()` call triggers
/// both a `Pragma { pragma_name: "X" }` action and a `Read` of the hidden
/// `pragma_X` table, so both arms of the authorizer consult these names.
const ALLOWED_PRAGMA_NAMES: [&str; 2] = ["page_count", "page_size"];

/// VM instructions between progress-handler invocations.
const PROGRESS_OPS: i32 = 10_000;

/// Maps a `rusqlite` error to a friendly timeout message when it was caused by
/// the progress handler interrupting a query past `deadline`; otherwise passes
/// the original error through. The `SQL query timed out` marker lets the backend
/// recognize this case and render guidance to the user.
fn map_query_err(err: rusqlite::Error, deadline: Option<Instant>) -> eyre::Report {
    match deadline {
        Some(deadline) if Instant::now() >= deadline => {
            eyre!("SQL query timed out. Run mevlog locally to query without limits.")
        }
        _ => err.into(),
    }
}

/// Authorizer callback: permit only read-only access to the allowed tables
/// plus SQL function calls. Reads of any other table error out, and every
/// mutating/structural/side-effecting action (ATTACH, DETACH, PRAGMA,
/// transactions, DDL, DML) is denied.
fn authorize(ctx: AuthContext<'_>) -> Authorization {
    match ctx.action {
        AuthAction::Select | AuthAction::Function { .. } | AuthAction::Recursive => {
            Authorization::Allow
        }
        AuthAction::Read { table_name, .. } => {
            let pragma_table = table_name
                .strip_prefix("pragma_")
                .is_some_and(|name| ALLOWED_PRAGMA_NAMES.contains(&name));
            if ALLOWED_TABLES.contains(&table_name) || pragma_table {
                Authorization::Allow
            } else {
                Authorization::Deny
            }
        }
        // Permit only the read-only DB-size pragmas, and only their
        // table-valued (no-value) form; any `PRAGMA name = value` is denied.
        AuthAction::Pragma {
            pragma_name,
            pragma_value: None,
        } if ALLOWED_PRAGMA_NAMES.contains(&pragma_name) => Authorization::Allow,
        _ => Authorization::Deny,
    }
}

/// Result of a raw SQL query: the selected column names (in `SELECT` order) plus
/// one JSON object per row. Columns are carried separately so tabular consumers
/// can render headers even when no rows are returned.
#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

/// Runs [`run_raw_query`] on the blocking pool so the SQLite work doesn't stall
/// the async runtime. Arguments are owned for the `'static` `spawn_blocking`
/// closure.
pub(crate) async fn run_raw_query_async(
    sql: String,
    db_path: String,
    max_rows: Option<usize>,
    timeout: Option<Duration>,
) -> Result<QueryResult> {
    tokio::task::spawn_blocking(move || run_raw_query(&sql, &db_path, max_rows, timeout))
        .await
        .map_err(|e| eyre!("query execution task failed: {e}"))?
}

/// Runs a user-provided SQL statement against the read-only txs DB and
/// serializes each result row into a JSON object keyed by column name.
/// Errors if the result exceeds `max_rows` (`None` = unlimited); rows are
/// stepped lazily, so nothing past the cap is ever materialized.
///
/// Uses a read-only `rusqlite` connection rather than `sqlx` so the custom
/// `u256_sum` SQL function is available to the query.
fn run_raw_query(
    sql: &str,
    db_path: &str,
    max_rows: Option<usize>,
    timeout: Option<Duration>,
) -> Result<QueryResult> {
    // Accept both `sqlite://<path>` URLs and bare filesystem paths.
    let filename = db_path
        .strip_prefix("sqlite://")
        .or_else(|| db_path.strip_prefix("sqlite:"))
        .unwrap_or(db_path);

    // No `SQLITE_OPEN_URI`: keeps `file:...?mode=rwc` tricks out of any filename
    // the SQL could reference. The path is resolved by us, not the user.
    let conn = Connection::open_with_flags(filename, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    // A C-API call, not a PRAGMA, so the authorizer (installed below) allows it.
    conn.busy_timeout(crate::db::shared::BUSY_TIMEOUT)?;
    register_functions(&conn)?;

    // Defense-in-depth on top of the read-only handle and the authorizer. Set
    // before the authorizer is installed, since the authorizer denies PRAGMA.
    conn.pragma_update(None, "query_only", true)?;

    // Interrupt runaway queries once the deadline passes. With no timeout the
    // query runs unbounded (trusted local CLI use).
    let deadline = timeout.map(|t| Instant::now() + t);
    if let Some(deadline) = deadline {
        conn.progress_handler(PROGRESS_OPS, Some(move || Instant::now() >= deadline));
    }

    conn.authorizer(Some(authorize));

    let mut stmt = conn.prepare(sql).map_err(|e| map_query_err(e, deadline))?;
    let columns: Vec<String> = stmt.column_names().iter().map(|c| c.to_string()).collect();

    // Rows are keyed by column name, so duplicate names would silently collapse
    // (last value wins). Reject them instead of emitting corrupt output.
    let mut seen = HashSet::with_capacity(columns.len());
    if let Some(dup) = columns.iter().find(|c| !seen.insert(c.as_str())) {
        bail!("query returns duplicate column name `{dup}`; alias columns to make them unique");
    }

    let col_count = columns.len();
    let mut out = Vec::new();
    let mut rows = stmt.query([]).map_err(|e| map_query_err(e, deadline))?;
    while let Some(row) = rows.next().map_err(|e| map_query_err(e, deadline))? {
        if let Some(max_rows) = max_rows
            && out.len() == max_rows
        {
            bail!("query returned more than {max_rows} rows; add a LIMIT clause");
        }
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
    async fn raw_query_interrupts_long_query_past_timeout() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;

        let txs: Vec<Transaction> = (0..40)
            .map(|i| Transaction {
                tx_index: i,
                tx_hash: FixedBytes::<32>::from([i as u8; 32]),
                ..sample_tx()
            })
            .collect();
        Transaction::save_batch(&txs, &write).await?;

        // A huge cross join over the allowed table runs far longer than the
        // timeout; the progress handler must interrupt it once it elapses.
        let sql = "SELECT count(*) FROM transactions a, transactions b, \
                   transactions c, transactions d, transactions e";
        let err = run_raw_query(sql, &path, None, Some(Duration::from_millis(50))).unwrap_err();
        assert!(
            err.to_string().contains("SQL query timed out"),
            "expected timeout, got: {err}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_encodes_blobs_as_hex_and_ints_as_numbers() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let result = run_raw_query(
            "SELECT block_number, tx_hash, from_address, signature FROM transactions",
            &path,
            None,
            None,
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
            None,
            None,
        )?;

        assert!(result.rows.is_empty());
        assert_eq!(result.columns, ["block_number", "tx_hash"]);

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_rejects_duplicate_column_names() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let err = run_raw_query("SELECT 1 AS x, 2 AS x FROM transactions", &path, None, None)
            .unwrap_err();
        assert!(err.to_string().contains("duplicate column name `x`"));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_supports_projection_and_aggregates() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let result = run_raw_query("SELECT COUNT(*) AS n FROM transactions", &path, None, None)?;
        assert_eq!(result.rows[0]["n"], json!(1));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_enforces_max_rows() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let sql = "SELECT block_number FROM transactions";
        assert!(run_raw_query(sql, &path, Some(1), None).is_ok());

        let err = run_raw_query(sql, &path, Some(0), None).unwrap_err();
        assert!(err.to_string().contains("more than 0 rows"));

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
            let err = run_raw_query(stmt, &path, None, None);
            assert!(err.is_err(), "expected `{stmt}` to be rejected");
        }

        let result = run_raw_query("SELECT COUNT(*) AS n FROM transactions", &path, None, None)?;
        assert_eq!(result.rows[0]["n"], json!(1));

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_allows_allowed_tables() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        for table in ["transactions", "logs", "blocks"] {
            let sql = format!("SELECT COUNT(*) AS n FROM {table}");
            assert!(
                run_raw_query(&sql, &path, None, None).is_ok(),
                "`{table}` should be readable"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_denies_non_allowed_tables() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        for sql in [
            "SELECT name FROM sqlite_master",
            "SELECT version FROM _sqlx_migrations",
        ] {
            assert!(
                run_raw_query(sql, &path, None, None).is_err(),
                "expected `{sql}` denied"
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_allows_db_size_pragma_functions() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        let result = run_raw_query(
            "SELECT (SELECT page_count FROM pragma_page_count()) \
             * (SELECT page_size FROM pragma_page_size()) AS bytes",
            &path,
            None,
            None,
        )?;

        assert_eq!(result.columns, ["bytes"]);
        let bytes = result.rows[0]["bytes"].as_i64().expect("integer bytes");
        assert!(bytes > 0, "db size should be positive, got {bytes}");

        Ok(())
    }

    #[tokio::test]
    async fn raw_query_denies_attach_and_pragma() -> Result<()> {
        let (write, path, _cl) = setup_test_db_rw().await;
        Transaction::save_batch(&[sample_tx()], &write).await?;

        for sql in [
            "ATTACH DATABASE 'file:/tmp/evil?mode=rwc' AS e",
            "PRAGMA query_only = OFF",
        ] {
            assert!(
                run_raw_query(sql, &path, None, None).is_err(),
                "expected `{sql}` denied"
            );
        }

        Ok(())
    }
}
