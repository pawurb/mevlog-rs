//! Config-defined custom tables in the per-chain txs DB, populated from
//! `logs` rows matching a `topic0`. Pure derived data: the `logs` table keeps
//! raw topics and data for every indexed block, so custom tables can always be
//! (re)built with a single `INSERT INTO ... SELECT` — no RPC re-fetch, and all
//! decoding lives in SQL.

use alloy::primitives::keccak256;
use eyre::{Result, bail};
use sqlx::SqlitePool;

use crate::misc::config::{ColumnSource, ColumnType, CustomColumn, CustomTable, valid_sql_name};

/// Reconciles configured custom tables with the DB state. Called on startup
/// for every command that opens the txs DB. Returns the tables applicable to
/// `chain_id` (the set the indexing path populates per chunk).
///
/// Per table: missing → create, record fingerprint, backfill from the full
/// existing `logs` table; fingerprint matches → no-op; mismatch (or an
/// untracked table squatting on the name) → error pointing at
/// `update-db --rebuild-tables`. Tables removed from config are left in place.
pub(crate) async fn sync(
    tables: &[CustomTable],
    chain_id: u64,
    pool: &SqlitePool,
) -> Result<Vec<CustomTable>> {
    let applicable: Vec<CustomTable> = tables
        .iter()
        .filter(|t| t.applies_to_chain(chain_id))
        .cloned()
        .collect();

    if applicable.is_empty() {
        return Ok(applicable);
    }

    ensure_meta_table(pool).await?;

    for table in &applicable {
        let fp = fingerprint(table);
        let exists = table_exists(&table.name, pool).await?;
        let stored: Option<String> =
            sqlx::query_scalar("SELECT fingerprint FROM custom_tables WHERE name = ?")
                .bind(&table.name)
                .fetch_optional(pool)
                .await?;

        match (exists, stored) {
            (true, Some(stored)) if stored == fp => {}
            (true, Some(_)) => bail!(
                "custom table '{}' no longer matches its config definition; \
                 run 'mevlog update-db --rebuild-tables --chain-id {chain_id}' \
                 to drop and rebuild it from indexed logs",
                table.name
            ),
            (true, None) => bail!(
                "table '{}' exists in the txs DB but is not a tracked custom table; \
                 rename it in config or run 'mevlog update-db --rebuild-tables --chain-id {chain_id}'",
                table.name
            ),
            (false, _) => create_and_backfill(table, pool).await?,
        }
    }

    Ok(applicable)
}

/// Drops every tracked custom table (including ones removed from config) plus
/// any table squatting on a configured name, then recreates and repopulates
/// the tables applicable to `chain_id` from `logs`. Lossless and offline.
/// Only this chain's DB is touched; multi-chain configs need one run per
/// chain. Returns the rebuilt table names.
pub(crate) async fn rebuild(
    tables: &[CustomTable],
    chain_id: u64,
    pool: &SqlitePool,
) -> Result<Vec<String>> {
    ensure_meta_table(pool).await?;

    let tracked: Vec<String> = sqlx::query_scalar("SELECT name FROM custom_tables")
        .fetch_all(pool)
        .await?;

    let applicable: Vec<&CustomTable> = tables
        .iter()
        .filter(|t| t.applies_to_chain(chain_id))
        .collect();

    let mut to_drop: Vec<String> = tracked;
    for table in &applicable {
        if !to_drop.contains(&table.name) {
            to_drop.push(table.name.clone());
        }
    }

    for name in &to_drop {
        // Tracked names normally passed config validation, but the meta table
        // is plain data — re-check before interpolating into DROP TABLE.
        if !valid_sql_name(name) {
            bail!("custom_tables meta row '{name}' is not a valid table name; refusing to drop");
        }
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "DROP TABLE IF EXISTS \"{name}\""
        )))
        .execute(pool)
        .await?;
    }
    sqlx::query("DELETE FROM custom_tables")
        .execute(pool)
        .await?;

    for table in &applicable {
        create_and_backfill(table, pool).await?;
    }

    Ok(applicable.iter().map(|t| t.name.clone()).collect())
}

/// Populates each table from the `logs` rows in `from..=to`. Run after every
/// indexing chunk lands, so custom tables stay in step with `logs`.
/// Idempotent: row identity is `(block_number, log_index)` with
/// `ON CONFLICT DO NOTHING`.
pub(crate) async fn populate_range(
    tables: &[CustomTable],
    from: u64,
    to: u64,
    pool: &SqlitePool,
) -> Result<()> {
    for table in tables {
        sqlx::query(sqlx::AssertSqlSafe(populate_sql(table, true)))
            .bind(from as i64)
            .bind(to as i64)
            .execute(pool)
            .await?;
    }
    Ok(())
}

/// Names of tracked custom tables, or empty when the meta table doesn't exist
/// (no custom tables were ever synced into this DB). Used by the purge path
/// to clean derived rows alongside `logs`.
pub(crate) async fn tracked_table_names(pool: &SqlitePool) -> Result<Vec<String>> {
    let meta_exists: Option<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'custom_tables'",
    )
    .fetch_optional(pool)
    .await?;

    if meta_exists.is_none() {
        return Ok(vec![]);
    }

    // Join against sqlite_master so a meta row whose table was dropped
    // manually doesn't produce a DELETE against a missing table.
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT ct.name FROM custom_tables ct \
         JOIN sqlite_master sm ON sm.name = ct.name AND sm.type = 'table'",
    )
    .fetch_all(pool)
    .await?;

    for name in &names {
        if !valid_sql_name(name) {
            bail!("custom_tables meta row '{name}' is not a valid table name");
        }
    }

    Ok(names)
}

async fn create_and_backfill(table: &CustomTable, pool: &SqlitePool) -> Result<()> {
    sqlx::query(sqlx::AssertSqlSafe(create_table_sql(table)))
        .execute(pool)
        .await?;

    sqlx::query(
        "INSERT INTO custom_tables (name, fingerprint) VALUES (?, ?) \
         ON CONFLICT(name) DO UPDATE SET fingerprint = excluded.fingerprint",
    )
    .bind(&table.name)
    .bind(fingerprint(table))
    .execute(pool)
    .await?;

    sqlx::query(sqlx::AssertSqlSafe(populate_sql(table, false)))
        .execute(pool)
        .await?;

    Ok(())
}

/// Created lazily at runtime, deliberately not via a sqlx migration — custom
/// tables are config-driven and can't live in static migrations, and this
/// avoids bumping `SCHEMA_VERSION` / the DB filename.
async fn ensure_meta_table(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS custom_tables (\
         name TEXT PRIMARY KEY, fingerprint TEXT NOT NULL)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

async fn table_exists(name: &str, pool: &SqlitePool) -> Result<bool> {
    let found: Option<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(name)
            .fetch_optional(pool)
            .await?;
    Ok(found.is_some())
}

/// Stable hash of the parts of a definition that determine the table's shape
/// and contents: topic0, sorted addresses, ordered columns. `chains` is
/// excluded — it only scopes which DBs get the table, not what's in it.
pub(crate) fn fingerprint(table: &CustomTable) -> String {
    let mut addresses: Vec<String> = table
        .addresses
        .iter()
        .map(|a| hex::encode(a.as_slice()))
        .collect();
    addresses.sort();

    let columns: Vec<String> = table
        .columns
        .iter()
        .map(|c| {
            format!(
                "{}:{}:{}",
                c.name,
                c.source.canonical(),
                c.r#type.canonical()
            )
        })
        .collect();

    let canonical = format!(
        "topic0={};addresses={};columns={}",
        hex::encode(table.topic0),
        addresses.join(","),
        columns.join("|"),
    );

    hex::encode(keccak256(canonical.as_bytes()))
}

fn create_table_sql(table: &CustomTable) -> String {
    let custom_columns: String = table
        .columns
        .iter()
        .map(|c| format!("    \"{}\" BLOB,\n", c.name))
        .collect();

    format!(
        "CREATE TABLE \"{}\" (\n\
         \x20   block_number BIGINT NOT NULL,\n\
         \x20   tx_index BIGINT NOT NULL,\n\
         \x20   log_index BIGINT NOT NULL,\n\
         \x20   address BLOB NOT NULL,\n\
         {custom_columns}\
         \x20   PRIMARY KEY (block_number, log_index)\n\
         )",
        table.name
    )
}

/// SELECT expression decoding one column from a `logs` row. SQLite `substr`
/// is 1-based; config byte ranges are 0-based.
fn column_expr(column: &CustomColumn) -> String {
    match (&column.source, column.r#type) {
        // Topics are 32 bytes; addresses live in the low 20.
        (ColumnSource::Topic(idx), ColumnType::Address) => format!("substr(topic{idx}, 13, 20)"),
        (ColumnSource::Topic(idx), _) => format!("topic{idx}"),
        (ColumnSource::Data { start, end }, ColumnType::Address) => {
            // 32-byte sources carry the 12-byte ABI pad; strip it.
            let offset = if end - start == 32 {
                start + 13
            } else {
                start + 1
            };
            format!("substr(data, {offset}, 20)")
        }
        (ColumnSource::Data { start, end }, ColumnType::Uint256) => {
            let len = end - start;
            let slice = format!("substr(data, {}, {len})", start + 1);
            if len == 32 {
                slice
            } else {
                // Left-pad to 32 bytes so u256_* functions and lexicographic
                // blob comparisons against 32-byte literals behave.
                format!("zeroblob(32 - length({slice})) || {slice}")
            }
        }
        (ColumnSource::Data { start, end }, ColumnType::Bytes) => {
            format!("substr(data, {}, {})", start + 1, end - start)
        }
    }
}

fn populate_sql(table: &CustomTable, bounded: bool) -> String {
    let names: String = table
        .columns
        .iter()
        .map(|c| format!(", \"{}\"", c.name))
        .collect();
    let exprs: String = table
        .columns
        .iter()
        .map(|c| format!(", {}", column_expr(c)))
        .collect();

    let mut sql = format!(
        "INSERT INTO \"{}\" (block_number, tx_index, log_index, address{names})\n\
         SELECT block_number, tx_index, log_index, address{exprs}\n\
         FROM logs\n\
         WHERE topic0 = X'{}'",
        table.name,
        hex::encode(table.topic0),
    );

    if !table.addresses.is_empty() {
        let addresses: Vec<String> = table
            .addresses
            .iter()
            .map(|a| format!("X'{}'", hex::encode(a.as_slice())))
            .collect();
        sql.push_str(&format!("\n  AND address IN ({})", addresses.join(", ")));
    }

    if bounded {
        sql.push_str("\n  AND block_number BETWEEN ? AND ?");
    }

    sql.push_str("\nON CONFLICT DO NOTHING");
    sql
}

#[cfg(test)]
mod test {
    use revm::primitives::{Address, FixedBytes, U256};
    use sqlx::Row;

    use super::*;
    use crate::{
        db::txs::models::{log::Log, transaction::test::setup_test_db},
        misc::config::Config,
    };

    const TOPIC0_HEX: &str = "d78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";
    const EMITTER: [u8; 20] = [0xb4; 20];

    fn tables_from_toml(toml_str: &str) -> Vec<CustomTable> {
        let config: Config = toml::from_str(toml_str).unwrap();
        config.custom_tables().unwrap()
    }

    fn swaps_table(extra: &str) -> CustomTable {
        let toml_str = format!(
            r#"
[tables.swaps]
topic0 = "0x{TOPIC0_HEX}"
{extra}

[[tables.swaps.columns]]
name = "sender"
source = "topic1"
type = "address"

[[tables.swaps.columns]]
name = "amount0_in"
source = "data[0:32]"
type = "uint256"

[[tables.swaps.columns]]
name = "to_address"
source = "topic2"
type = "address"
"#
        );
        tables_from_toml(&toml_str).remove(0)
    }

    fn swap_log(block_number: u64, log_index: u64, emitter: [u8; 20], amount: u64) -> Log {
        let pad_address = |byte: u8| {
            let mut topic = [0u8; 32];
            topic[12..].copy_from_slice(&[byte; 20]);
            FixedBytes::<32>::from(topic)
        };

        Log {
            block_number,
            tx_index: 0,
            log_index,
            address: Address::from(emitter),
            topics: vec![
                FixedBytes::<32>::from_slice(&hex::decode(TOPIC0_HEX).unwrap()),
                pad_address(0xaa),
                pad_address(0xcc),
            ],
            data: U256::from(amount).to_be_bytes::<32>().to_vec(),
            erc20_amount: None,
            signature: None,
        }
    }

    async fn row_count(table: &str, conn: &sqlx::SqlitePool) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
            .fetch_one(conn)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn sync_creates_table_and_backfills_existing_logs() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut other = swap_log(100, 1, EMITTER, 7);
        other.topics[0] = FixedBytes::<32>::from([0xee; 32]);
        Log::save_batch(&[swap_log(100, 0, EMITTER, 500), other], &conn).await?;

        let table = swaps_table("");
        let applicable = sync(std::slice::from_ref(&table), 1, &conn).await?;
        assert_eq!(applicable.len(), 1);

        let rows = sqlx::query("SELECT * FROM swaps ORDER BY block_number, log_index")
            .fetch_all(&conn)
            .await?;
        assert_eq!(rows.len(), 1);

        let row = &rows[0];
        assert_eq!(row.get::<i64, _>("block_number"), 100);
        assert_eq!(row.get::<Vec<u8>, _>("address"), EMITTER.to_vec());
        // Topic-sourced addresses get the 12-byte pad stripped.
        assert_eq!(row.get::<Vec<u8>, _>("sender"), vec![0xaa; 20]);
        assert_eq!(row.get::<Vec<u8>, _>("to_address"), vec![0xcc; 20]);
        assert_eq!(
            row.get::<Vec<u8>, _>("amount0_in"),
            U256::from(500u64).to_be_bytes::<32>().to_vec()
        );

        // Re-sync with an unchanged definition is a no-op.
        sync(std::slice::from_ref(&table), 1, &conn).await?;
        assert_eq!(row_count("swaps", &conn).await, 1);

        Ok(())
    }

    #[tokio::test]
    async fn sync_skips_tables_scoped_to_other_chains() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let table = swaps_table("chains = [10]");
        let applicable = sync(std::slice::from_ref(&table), 1, &conn).await?;

        assert!(applicable.is_empty());
        assert!(!table_exists("swaps", &conn).await?);

        Ok(())
    }

    #[tokio::test]
    async fn sync_errors_on_fingerprint_mismatch() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let table = swaps_table("");
        sync(std::slice::from_ref(&table), 1, &conn).await?;

        let mut changed = table.clone();
        changed.columns.pop();
        let err = sync(std::slice::from_ref(&changed), 1, &conn)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("--rebuild-tables"), "unexpected error: {err}");

        Ok(())
    }

    #[tokio::test]
    async fn sync_errors_on_untracked_table_with_same_name() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        sqlx::query("CREATE TABLE swaps (id INTEGER PRIMARY KEY)")
            .execute(&conn)
            .await?;

        let table = swaps_table("");
        let err = sync(std::slice::from_ref(&table), 1, &conn)
            .await
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("not a tracked custom table"),
            "unexpected error: {err}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn populate_range_is_bounded_and_idempotent() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let table = swaps_table("");
        let applicable = sync(std::slice::from_ref(&table), 1, &conn).await?;

        Log::save_batch(
            &[swap_log(100, 0, EMITTER, 1), swap_log(101, 0, EMITTER, 2)],
            &conn,
        )
        .await?;

        populate_range(&applicable, 100, 100, &conn).await?;
        assert_eq!(row_count("swaps", &conn).await, 1);

        populate_range(&applicable, 100, 100, &conn).await?;
        assert_eq!(row_count("swaps", &conn).await, 1);

        populate_range(&applicable, 100, 101, &conn).await?;
        assert_eq!(row_count("swaps", &conn).await, 2);

        Ok(())
    }

    #[tokio::test]
    async fn address_filter_limits_captured_logs() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        Log::save_batch(
            &[
                swap_log(100, 0, EMITTER, 1),
                swap_log(100, 1, [0x99; 20], 2),
            ],
            &conn,
        )
        .await?;

        let table = swaps_table(&format!("addresses = [\"0x{}\"]", hex::encode(EMITTER)));
        sync(std::slice::from_ref(&table), 1, &conn).await?;

        assert_eq!(row_count("swaps", &conn).await, 1);
        let address: Vec<u8> = sqlx::query_scalar("SELECT address FROM swaps")
            .fetch_one(&conn)
            .await?;
        assert_eq!(address, EMITTER.to_vec());

        Ok(())
    }

    #[tokio::test]
    async fn short_uint256_data_range_is_left_padded() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut log = swap_log(100, 0, EMITTER, 0);
        log.data = vec![0x01, 0x02, 0x03, 0x04];
        Log::save_batch(std::slice::from_ref(&log), &conn).await?;

        let toml_str = format!(
            r#"
[tables.shorts]
topic0 = "0x{TOPIC0_HEX}"

[[tables.shorts.columns]]
name = "amount"
source = "data[0:4]"
type = "uint256"
"#
        );
        let tables = tables_from_toml(&toml_str);
        sync(&tables, 1, &conn).await?;

        let amount: Vec<u8> = sqlx::query_scalar("SELECT amount FROM shorts")
            .fetch_one(&conn)
            .await?;
        let mut expected = vec![0u8; 28];
        expected.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(amount, expected);

        Ok(())
    }

    #[tokio::test]
    async fn rebuild_drops_stale_tables_and_recreates_configured_ones() -> eyre::Result<()> {
        let (conn, _cl) = setup_test_db().await;

        Log::save_batch(&[swap_log(100, 0, EMITTER, 500)], &conn).await?;

        // Old config: 3 columns. New config: address filter excluding the
        // emitter, so the rebuilt table must be empty.
        let old = swaps_table("");
        sync(std::slice::from_ref(&old), 1, &conn).await?;
        assert_eq!(row_count("swaps", &conn).await, 1);

        let new = swaps_table("addresses = [\"0x9999999999999999999999999999999999999999\"]");
        let rebuilt = rebuild(std::slice::from_ref(&new), 1, &conn).await?;
        assert_eq!(rebuilt, vec!["swaps".to_string()]);
        assert_eq!(row_count("swaps", &conn).await, 0);

        // A subsequent sync with the new definition passes (fingerprint updated).
        sync(std::slice::from_ref(&new), 1, &conn).await?;

        // Rebuild with the table removed from config drops it entirely.
        let rebuilt = rebuild(&[], 1, &conn).await?;
        assert!(rebuilt.is_empty());
        assert!(!table_exists("swaps", &conn).await?);
        assert_eq!(tracked_table_names(&conn).await?, Vec::<String>::new());

        Ok(())
    }

    #[test]
    fn fingerprint_ignores_address_order_and_chains_but_not_column_order() {
        let base = swaps_table(
            "addresses = [\"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\", \"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\"]",
        );
        let reordered = swaps_table(
            "addresses = [\"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\", \"0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\"]",
        );
        assert_eq!(fingerprint(&base), fingerprint(&reordered));

        let mut other_chains = base.clone();
        other_chains.chains = Some(vec![42161]);
        assert_eq!(fingerprint(&base), fingerprint(&other_chains));

        let mut swapped = base.clone();
        swapped.columns.swap(0, 1);
        assert_ne!(fingerprint(&base), fingerprint(&swapped));
    }
}
