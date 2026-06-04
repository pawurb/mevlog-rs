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
        models::json::query_response::QueryResponse,
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

        let output = run_query(&rpc_url, &tmp_dir, None, "json");

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

        let expected = "block_number,tx_index,tx_hash,nonce,from_address,to_address,value,gas_limit,gas_used,effective_gas_price,gas_price,max_fee_per_gas,max_priority_fee_per_gas,transaction_type,success,signature_hash,signature\n\
            25215353,0,0xdfe463a0a9fdd80ec3de153fef56e9f57ac7437ac7d7ab7276014017b8bc19e5,7366,0xf34f8b87f3db3b3a664289b4b063b507535eced1,0x80a64c6d7f12c47b7c66c5b4e20e72bc1fcd5d9e,0x0000000000000000000000000000000000000000000000000000000000000000,336986,157961,3133334821,3133334821,3191972299,3000000000,2,1,0x3d0e3ec5,\"swapExactTokensForETHSupportingFeeOnTransferTokens(uint256,uint256,address[],address,uint256,address)\"\n";

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
            format!("{FROM_BLOCK}:{TO_BLOCK}"),
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

        assert_eq!(stdout.trim_end(), expected, "table output mismatch");

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

        let first = run_query(&rpc_url, &tmp_dir, None, "json");
        assert!(
            first.status.success(),
            "first query failed: {}",
            String::from_utf8_lossy(&first.stderr),
        );
        let first_json: QueryResponse = serde_json::from_slice(&first.stdout)?;
        assert_eq!(first_json.cached_blocks, 0, "first run cached_blocks");
        assert_eq!(first_json.new_blocks, total_blocks, "first run new_blocks");

        let second = run_query(&rpc_url, &tmp_dir, None, "json");
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
}
