#[cfg(test)]
pub mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    };

    use eyre::Result;
    use mevlog::{
        db::txs::{
            self,
            models::{log::Log, transaction::Transaction},
        },
        models::json::{
            block_json::BlockJson, db_info_response::DbInfoResponse, log_json::LogJson,
            purge_response::PurgeResponse, query_response::QueryResponse,
            transaction_json::TransactionJson,
        },
    };
    use uuid::Uuid;

    const CHAIN_ID: u64 = 1;
    const FROM_BLOCK: u64 = 25215353;
    const TO_BLOCK: u64 = 25215357;

    const TX_HASH: &str = "a03753bac3008c6fe05ed2f95045995632db4bd332426e9b823b3a73d0dd8594";
    const TX_SELECTOR: &str = "a9059cbb";
    const TX_SIGNATURE: &str = "transfer(address,uint256)";

    const RESOLVED_LOG_BLOCK: u64 = 25215353;
    const RESOLVED_LOG_INDEX: u64 = 0;
    const RESOLVED_LOG_SIGNATURE: &str = "Transfer(address,address,uint256)";

    const UNKNOWN_LOG_BLOCK: u64 = 25215353;
    const UNKNOWN_LOG_INDEX: u64 = 274;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cryo/ethereum")
    }

    fn cryo_cache_dir() -> PathBuf {
        home::home_dir()
            .expect("home dir")
            .join(".mevlog/.cryo-cache/ethereum")
    }

    fn sync_fixtures_to_cache() {
        let cache = cryo_cache_dir();
        fs::create_dir_all(&cache).expect("create cryo cache dir");

        for entry in fs::read_dir(fixtures_dir()).expect("read fixtures dir") {
            let path = entry.expect("fixture entry").path();
            let name = path.file_name().expect("fixture file name");
            fs::copy(&path, cache.join(name)).expect("copy fixture into cache");
        }
    }

    fn run_query(rpc_url: &str, tmp_dir: &Path, sql: Option<&str>, format: &str) -> Output {
        run_query_with_price(rpc_url, tmp_dir, sql, format, "1")
    }

    fn run_query_with_price(
        rpc_url: &str,
        tmp_dir: &Path,
        sql: Option<&str>,
        format: &str,
        native_token_price: &str,
    ) -> Output {
        let mut command = Command::new("cargo");
        command
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "query"])
            .args(["-b", &format!("{FROM_BLOCK}:{TO_BLOCK}")])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--rpc-url", rpc_url])
            .arg("--skip-verify-chain-id")
            .args(["--native-token-price", native_token_price])
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", format]);

        if let Some(sql) = sql {
            command.args(["--sql", sql]);
        }

        command.output().expect("failed to execute CLI")
    }

    fn run_tx(rpc_url: &str, tmp_dir: &Path, tx_hash: &str, native_token_price: &str) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "tx", tx_hash])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--rpc-url", rpc_url])
            .arg("--skip-verify-chain-id")
            .args(["--native-token-price", native_token_price])
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", "json"])
            .output()
            .expect("failed to execute CLI")
    }

    fn run_tx_logs(rpc_url: &str, tmp_dir: &Path, tx_hash: &str) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "tx-logs", tx_hash])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--rpc-url", rpc_url])
            .arg("--skip-verify-chain-id")
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", "json"])
            .output()
            .expect("failed to execute CLI")
    }

    fn run_block(rpc_url: &str, tmp_dir: &Path, block: &str) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "block", "-b", block])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--rpc-url", rpc_url])
            .arg("--skip-verify-chain-id")
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", "json"])
            .output()
            .expect("failed to execute CLI")
    }

    fn run_block_txs(
        rpc_url: &str,
        tmp_dir: &Path,
        block: &str,
        native_token_price: &str,
    ) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "block-txs", "-b", block])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--rpc-url", rpc_url])
            .arg("--skip-verify-chain-id")
            .args(["--native-token-price", native_token_price])
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", "json"])
            .output()
            .expect("failed to execute CLI")
    }

    fn run_purge(tmp_dir: &Path, keep: u64) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "purge-db"])
            .args(["--keep", &keep.to_string()])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", "json"])
            .output()
            .expect("failed to execute CLI")
    }

    fn run_db_info(tmp_dir: &Path) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .env("MEVLOG_KEEP_CRYO_CACHE", "1")
            .args(["run", "--bin", "mevlog", "--", "db-info"])
            .args(["--chain-id", &CHAIN_ID.to_string()])
            .args(["--txs-db-dir", &tmp_dir.to_string_lossy()])
            .args(["--format", "json"])
            .output()
            .expect("failed to execute CLI")
    }

    #[tokio::test]
    async fn test_db_info_and_purge_removes_indexed_data() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_query(&rpc_url, &tmp_dir, Some("SELECT 1"), "json");
        assert!(
            output.status.success(),
            "query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let db_path = tmp_dir.join(txs::db_file_name(txs::SCHEMA_VERSION, CHAIN_ID));
        let conn = txs::conn(
            Some(db_path.to_string_lossy().into_owned()),
            CHAIN_ID,
            false,
        )
        .await?;

        let total_blocks = TO_BLOCK - FROM_BLOCK + 1;
        let txs_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM transactions")
            .fetch_one(&conn)
            .await?;
        let logs_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
            .fetch_one(&conn)
            .await?;
        assert!(txs_count > 0, "transactions should be indexed");
        assert!(logs_count > 0, "logs should be indexed");

        let output = run_db_info(&tmp_dir);
        assert!(
            output.status.success(),
            "db-info failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let resp: DbInfoResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(resp.chain_id, CHAIN_ID, "chain id mismatch");
        assert_eq!(resp.schema_version, txs::SCHEMA_VERSION, "schema mismatch");
        assert_eq!(resp.db_path, db_path.to_string_lossy(), "db_path mismatch");
        assert_eq!(resp.blocks, total_blocks, "blocks mismatch");
        assert_eq!(resp.transactions, txs_count as u64, "txs mismatch");
        assert_eq!(resp.logs, logs_count as u64, "logs mismatch");
        assert_eq!(resp.min_block, Some(FROM_BLOCK), "min_block mismatch");
        assert_eq!(resp.max_block, Some(TO_BLOCK), "max_block mismatch");
        assert_eq!(resp.missing_blocks, 0, "missing_blocks mismatch");
        assert!(resp.db_size_bytes > 0, "db_size_bytes should be positive");
        assert!(
            resp.min_block_timestamp.is_some() && resp.min_block_time.is_some(),
            "min block timestamps should be set"
        );
        assert!(
            resp.max_block_timestamp >= resp.min_block_timestamp,
            "timestamps should be ordered"
        );

        // keep=0: everything up to and including the highest indexed block
        // (TO_BLOCK) is purged; the cutoff never consults the chain head.
        let output = run_purge(&tmp_dir, 0);
        assert!(
            output.status.success(),
            "purge failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let resp: PurgeResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(resp.keep, 0, "keep mismatch");
        assert_eq!(resp.latest_block, Some(TO_BLOCK), "latest_block mismatch");
        assert_eq!(resp.cutoff_block, Some(TO_BLOCK + 1), "cutoff mismatch");
        assert_eq!(resp.purged_blocks, total_blocks, "purged_blocks mismatch");
        assert_eq!(
            resp.purged_transactions, txs_count as u64,
            "purged_transactions mismatch"
        );
        assert_eq!(resp.purged_logs, logs_count as u64, "purged_logs mismatch");
        assert_eq!(resp.chain_id, CHAIN_ID, "chain id mismatch");

        let output = run_db_info(&tmp_dir);
        assert!(
            output.status.success(),
            "db-info after purge failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let resp: DbInfoResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(resp.blocks, 0, "blocks should be empty after purge");
        assert_eq!(resp.transactions, 0, "txs should be empty after purge");
        assert_eq!(resp.logs, 0, "logs should be empty after purge");
        assert_eq!(resp.min_block, None, "min_block should be null after purge");
        assert_eq!(resp.max_block, None, "max_block should be null after purge");
        assert_eq!(resp.min_block_time, None, "min_block_time should be null");
        assert_eq!(resp.max_block_time, None, "max_block_time should be null");
        assert_eq!(resp.missing_blocks, 0, "missing_blocks should be 0");
        assert!(resp.db_size_bytes > 0, "db file should still exist");

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_indexes_txs_into_sqlite() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let db_path = tmp_dir.join(txs::db_file_name(txs::SCHEMA_VERSION, CHAIN_ID));
        for suffix in ["", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", db_path.to_string_lossy()));
            fs::remove_file(&p).ok();
        }

        let output = run_query(&rpc_url, &tmp_dir, Some("SELECT 1"), "json");

        assert!(
            output.status.success(),
            "query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let conn = txs::conn(
            Some(db_path.to_string_lossy().into_owned()),
            CHAIN_ID,
            false,
        )
        .await?;

        let expected: Vec<u64> = (FROM_BLOCK..=TO_BLOCK).collect();

        let indexed: Vec<u64> =
            sqlx::query_scalar::<_, i64>("SELECT block_number FROM blocks ORDER BY block_number")
                .fetch_all(&conn)
                .await?
                .into_iter()
                .map(|b| b as u64)
                .collect();

        let tx_blocks: Vec<u64> = sqlx::query_scalar::<_, i64>(
            "SELECT DISTINCT block_number FROM transactions ORDER BY block_number",
        )
        .fetch_all(&conn)
        .await?
        .into_iter()
        .map(|b| b as u64)
        .collect();

        let logs_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM logs")
            .fetch_one(&conn)
            .await?;

        assert_eq!(indexed, expected, "blocks table mismatch");
        assert_eq!(tx_blocks, expected, "transaction blocks mismatch");
        assert_eq!(logs_count, 3868, "logs count mismatch");

        let txs = Transaction::query_where(&format!("tx_hash = x'{TX_HASH}'"), &conn).await?;
        assert_eq!(txs.len(), 1, "expected exactly one tx for {TX_HASH}");
        let tx = &txs[0];
        assert_eq!(
            tx.signature_hash.map(hex::encode).as_deref(),
            Some(TX_SELECTOR),
            "tx signature_hash mismatch"
        );
        assert_eq!(
            tx.signature.as_deref(),
            Some(TX_SIGNATURE),
            "tx signature mismatch"
        );

        let logs = Log::query_where(
            &format!("block_number = {RESOLVED_LOG_BLOCK} AND log_index = {RESOLVED_LOG_INDEX}"),
            &conn,
        )
        .await?;
        assert_eq!(logs.len(), 1, "expected exactly one resolved log");
        assert_eq!(
            logs[0].signature.as_deref(),
            Some(RESOLVED_LOG_SIGNATURE),
            "resolved log signature mismatch"
        );

        let unknown_logs = Log::query_where(
            &format!("block_number = {UNKNOWN_LOG_BLOCK} AND log_index = {UNKNOWN_LOG_INDEX}"),
            &conn,
        )
        .await?;
        assert_eq!(unknown_logs.len(), 1, "expected exactly one unknown log");
        assert_eq!(
            unknown_logs[0].signature, None,
            "unknown log signature should be NULL"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_custom_sql_csv_exact_output() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_query(
            &rpc_url,
            &tmp_dir,
            Some("SELECT * FROM transactions ORDER BY block_number ASC, tx_index ASC LIMIT 1"),
            "csv",
        );

        assert!(
            output.status.success(),
            "query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected = "block_number,tx_index,tx_hash,nonce,from_address,to_address,value,gas_limit,gas_used,effective_gas_price,gas_price,max_fee_per_gas,max_priority_fee_per_gas,transaction_type,success,coinbase_transfer,signature_hash,signature\n\
            25215353,0,0xdfe463a0a9fdd80ec3de153fef56e9f57ac7437ac7d7ab7276014017b8bc19e5,7366,0xf34f8b87f3db3b3a664289b4b063b507535eced1,0x80a64c6d7f12c47b7c66c5b4e20e72bc1fcd5d9e,0x0000000000000000000000000000000000000000000000000000000000000000,336986,157961,3133334821,3133334821,3191972299,3000000000,2,1,,0x3d0e3ec5,\"swapExactTokensForETHSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256,address)\"\n";

        assert_eq!(stdout, expected, "csv output mismatch");

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_custom_sql_json_envelope() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let sql = "SELECT * FROM transactions ORDER BY block_number ASC, tx_index ASC LIMIT 1";
        let output = run_query(&rpc_url, &tmp_dir, Some(sql), "json");

        assert!(
            output.status.success(),
            "query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;

        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert_eq!(envelope.result.len(), 1, "result array length mismatch");
        assert_eq!(
            envelope.query.sql.as_deref(),
            Some(sql),
            "echoed sql mismatch"
        );
        assert_eq!(
            envelope.query.blocks,
            Some(format!("{FROM_BLOCK}:{TO_BLOCK}")),
            "echoed blocks mismatch"
        );
        assert_eq!(envelope.chain.chain_id, CHAIN_ID, "chain id mismatch");
        assert!(!envelope.duration.is_empty(), "duration should be present");

        let row = &envelope.result[0];
        assert_eq!(row["block_number"], 25215353, "row block_number mismatch");
        assert_eq!(row["tx_index"], 0, "row tx_index mismatch");
        assert_eq!(
            row["tx_hash"], "0xdfe463a0a9fdd80ec3de153fef56e9f57ac7437ac7d7ab7276014017b8bc19e5",
            "row tx_hash mismatch"
        );
        assert_eq!(
            row["value"], "0x0000000000000000000000000000000000000000000000000000000000000000",
            "row value mismatch"
        );
        assert_eq!(
            row["signature"],
            "swapExactTokensForETHSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256,address)",
            "row signature mismatch"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_custom_sql_table_exact_output() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_query(
            &rpc_url,
            &tmp_dir,
            Some(
                "SELECT block_number, tx_index, signature FROM transactions \
                 ORDER BY block_number ASC, tx_index ASC LIMIT 1",
            ),
            "table",
        );

        assert!(
            output.status.success(),
            "query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected = "\
+--------------+----------+-------------------------------------------------------------------------------------------------------+
| block_number | tx_index | signature                                                                                             |
+=================================================================================================================================+
| 25215353     | 0        | swapExactTokensForETHSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256,address) |
+--------------+----------+-------------------------------------------------------------------------------------------------------+";

        // `contains` rather than exact equality: table output ends with a
        // volatile `generated_at: <timestamp>` line.
        assert!(stdout.contains(expected), "table output mismatch: {stdout}");

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    // Sum of all tx `value`s across blocks 25215353..=25215357, as a 32-byte
    // big-endian blob.
    const EXPECTED_VALUE_SUM: &str =
        "0x0000000000000000000000000000000000000000000000104ad26530a649aca2";

    #[tokio::test]
    async fn test_query_u256_sum_of_tx_values() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        // Index the range and sum the `value` column via the custom u256_sum
        // SQLite aggregate registered on the rusqlite read connection.
        let sum_out = run_query(
            &rpc_url,
            &tmp_dir,
            Some("SELECT u256_sum(value) AS total FROM transactions"),
            "json",
        );
        assert!(
            sum_out.status.success(),
            "sum query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&sum_out.stdout),
            String::from_utf8_lossy(&sum_out.stderr),
        );
        let sum_env: QueryResponse = serde_json::from_slice(&sum_out.stdout)?;
        assert_eq!(sum_env.result_count, 1, "sum should return a single row");
        assert_eq!(
            sum_env.result[0]["total"], EXPECTED_VALUE_SUM,
            "u256_sum(value) mismatch"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    // vitalik.eth, resolved via the ENS lookup oracle on mainnet.
    const VITALIK_ENS: &str = "vitalik.eth";
    const VITALIK_ADDR: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";

    #[tokio::test]
    async fn test_query_resolves_ens_macro() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        // {RESOLVE_ENS("vitalik.eth")} is substituted with the resolved address as a
        // blob literal before the SQL runs; selecting it back renders as 0x-hex.
        let output = run_query(
            &rpc_url,
            &tmp_dir,
            Some(&format!(
                "SELECT {{RESOLVE_ENS(\"{VITALIK_ENS}\")}} AS addr"
            )),
            "json",
        );
        assert!(
            output.status.success(),
            "ens query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert_eq!(
            envelope.result[0]["addr"], VITALIK_ADDR,
            "resolved ENS address mismatch"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_substitutes_latest_block_macro() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        // {LATEST_BLOCK()} is replaced with the current chain head (fetched via
        // RPC) before the SQL runs; the value is dynamic, so assert it's an integer.
        let output = run_query(
            &rpc_url,
            &tmp_dir,
            Some("SELECT {LATEST_BLOCK()} AS block"),
            "json",
        );
        assert!(
            output.status.success(),
            "latest block query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert!(
            envelope.result[0]["block"].is_u64(),
            "latest block should be an integer, got {}",
            envelope.result[0]["block"]
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_substitutes_native_token_price_macro() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        // {NATIVE_TOKEN_PRICE()} is replaced with the --native-token-price value
        // before the SQL runs; a decimal price round-trips as a JSON number.
        let output = run_query_with_price(
            &rpc_url,
            &tmp_dir,
            Some("SELECT {NATIVE_TOKEN_PRICE()} AS price"),
            "json",
            "2500.5",
        );
        assert!(
            output.status.success(),
            "price query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert_eq!(
            envelope.result[0]["price"], 2500.5,
            "native token price mismatch"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_query_reports_cached_blocks_on_repeat_run() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let total_blocks = TO_BLOCK - FROM_BLOCK + 1;

        let first = run_query(&rpc_url, &tmp_dir, Some("SELECT 1"), "json");
        assert!(
            first.status.success(),
            "first query failed: {}",
            String::from_utf8_lossy(&first.stderr),
        );
        let first_json: QueryResponse = serde_json::from_slice(&first.stdout)?;
        assert_eq!(first_json.cached_blocks, 0, "first run cached_blocks");
        assert_eq!(first_json.new_blocks, total_blocks, "first run new_blocks");

        let second = run_query(&rpc_url, &tmp_dir, Some("SELECT 1"), "json");
        assert!(
            second.status.success(),
            "second query failed: {}",
            String::from_utf8_lossy(&second.stderr),
        );
        let second_json: QueryResponse = serde_json::from_slice(&second.stdout)?;
        assert_eq!(
            second_json.cached_blocks, total_blocks,
            "second run cached_blocks"
        );
        assert_eq!(second_json.new_blocks, 0, "second run new_blocks");

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_tx_command_returns_exact_payload() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_tx(&rpc_url, &tmp_dir, TX_HASH, "2500.5");

        assert!(
            output.status.success(),
            "tx failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        // tx is a convenience wrapper around query: it emits the same envelope,
        // with the single matching transaction in `result`.
        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert_eq!(envelope.result.len(), 1, "result array length mismatch");
        assert_eq!(
            envelope.query.blocks,
            Some(FROM_BLOCK.to_string()),
            "echoed blocks mismatch"
        );
        assert_eq!(envelope.chain.chain_id, CHAIN_ID, "chain id mismatch");
        assert_eq!(
            envelope.chain.native_token_price,
            Some(2500.5),
            "native token price mismatch"
        );

        let payload = &envelope.result[0];

        // No --evm-trace, so coinbase/full-cost fields are null. USD fields use
        // the supplied --native-token-price (2500.5). Logs are served by `tx-logs`,
        // not embedded here.
        let expected = serde_json::json!({
            "block_number": 25215353,
            "tx_index": 2,
            "tx_hash": "0xa03753bac3008c6fe05ed2f95045995632db4bd332426e9b823b3a73d0dd8594",
            "from": "0x142f11cfb8a7bf975565a875ffe425c1216af7b6",
            "to": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "nonce": 12,
            "signature": "transfer(address,uint256)",
            "signature_hash": "0xa9059cbb",
            // SQLite has no boolean type; success is rendered as 0/1.
            "success": 1,
            "value": "0",
            "display_value": "0.000000 ETH",
            "gas_used": 63209,
            "gas_price": 2233334825u64,
            "display_gas_price": "2.23 gwei",
            "tx_cost": "141166860953425",
            "display_tx_cost": "0.000141 ETH",
            "display_tx_cost_usd": "$0.35",
            "coinbase_transfer": null,
            "display_coinbase_transfer": null,
            "display_coinbase_transfer_usd": null,
            "full_tx_cost": null,
            "display_full_tx_cost": null,
            "display_full_tx_cost_usd": null
        });

        assert_eq!(payload, &expected, "tx payload mismatch");

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_tx_logs_command_returns_logs() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_tx_logs(&rpc_url, &tmp_dir, TX_HASH);

        assert!(
            output.status.success(),
            "tx-logs failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        // tx-logs is a convenience wrapper around query: it emits the same
        // envelope, with the transaction's log rows in `result`. Topics are kept
        // as raw topic0..topic3 columns (NULL when absent), faithful to query.sql.
        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert_eq!(
            envelope.query.blocks,
            Some(FROM_BLOCK.to_string()),
            "echoed blocks mismatch"
        );
        assert_eq!(envelope.chain.chain_id, CHAIN_ID, "chain id mismatch");

        let expected = serde_json::json!({
            "log_index": 6,
            "address": "0xdac17f958d2ee523a2206206994597c13d831ec7",
            "signature": "Transfer(address,address,uint256)",
            "topic0": "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            "topic1": "0x000000000000000000000000142f11cfb8a7bf975565a875ffe425c1216af7b6",
            "topic2": "0x0000000000000000000000000f4d84b3c4a344c573b640a2f085b1f92013eedd",
            "topic3": null,
            "data": "0x000000000000000000000000000000000000000000000000000000002f9fc5c0",
            "erc20_amount": "799000000"
        });

        assert_eq!(&envelope.result[0], &expected, "tx-logs payload mismatch");

        // The row must also deserialize into the LogJson contract the TUI consumes.
        let log: LogJson = serde_json::from_value(envelope.result[0].clone())?;
        assert_eq!(log.log_index, 6, "log_index mismatch");
        assert_eq!(
            log.signature.as_deref(),
            Some(RESOLVED_LOG_SIGNATURE),
            "log signature mismatch"
        );
        assert!(
            log.topic1.is_some() && log.topic2.is_some() && log.topic3.is_none(),
            "ERC20 transfer should have topic1/topic2 set and topic3 empty"
        );
        assert_eq!(
            log.erc20_amount.as_deref(),
            Some("799000000"),
            "erc20_amount mismatch"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_block_command_returns_metadata() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_block(&rpc_url, &tmp_dir, &FROM_BLOCK.to_string());

        assert!(
            output.status.success(),
            "block failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        // block is a convenience wrapper around query: it emits the same envelope,
        // with the single block's metadata in `result`.
        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert_eq!(envelope.result_count, 1, "result_count mismatch");
        assert_eq!(envelope.chain.chain_id, CHAIN_ID, "chain id mismatch");

        let block: BlockJson = serde_json::from_value(envelope.result[0].clone())?;
        assert_eq!(block.block_number, FROM_BLOCK, "block_number mismatch");
        assert!(
            block.txs_count > 0,
            "block should have indexed transactions"
        );
        // Post-EIP-1559 mainnet block: base fee present and rendered in gwei.
        assert!(
            block
                .display_base_fee_per_gas
                .as_deref()
                .is_some_and(|s| s.ends_with(" gwei")),
            "display_base_fee_per_gas should be rendered in gwei, got {:?}",
            block.display_base_fee_per_gas
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }

    #[tokio::test]
    async fn test_block_txs_command_returns_txs() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let output = run_block_txs(&rpc_url, &tmp_dir, &FROM_BLOCK.to_string(), "2500.5");

        assert!(
            output.status.success(),
            "block-txs failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        // block-txs is a convenience wrapper around query: it emits the same
        // envelope, with the block's display-shaped transactions in `result`.
        let envelope: QueryResponse = serde_json::from_slice(&output.stdout)?;
        assert!(envelope.result_count > 0, "expected transactions in block");
        assert_eq!(
            envelope.query.blocks,
            Some(FROM_BLOCK.to_string()),
            "echoed blocks mismatch"
        );

        let txs: Vec<TransactionJson> = envelope
            .result
            .iter()
            .map(|row| serde_json::from_value(row.clone()))
            .collect::<std::result::Result<_, _>>()?;

        assert!(
            txs.iter().all(|tx| tx.block_number == FROM_BLOCK),
            "all rows should belong to the requested block"
        );

        let known = txs
            .iter()
            .find(|tx| tx.tx_hash.to_string() == format!("0x{TX_HASH}"))
            .expect("the known transfer tx should be present in the block");
        assert_eq!(known.tx_index, 2, "known tx index mismatch");
        assert_eq!(known.signature, TX_SIGNATURE, "known tx signature mismatch");

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }
}
