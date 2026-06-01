#[cfg(test)]
pub mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Output},
    };

    use eyre::Result;
    use mevlog::db::txs::{
        self,
        models::{log::Log, transaction::Transaction},
    };
    use serde::Deserialize;
    use uuid::Uuid;

    #[derive(Deserialize)]
    struct QueryResponse {
        cached_blocks: u64,
        new_blocks: u64,
    }

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

    fn run_query(rpc_url: &str, tmp_dir: &Path) -> Output {
        Command::new("cargo")
            .env("RUST_LOG", "off")
            .args([
                "run",
                "--bin",
                "mevlog",
                "--",
                "query",
                "-b",
                &format!("{FROM_BLOCK}:{TO_BLOCK}"),
                "--chain-id",
                &CHAIN_ID.to_string(),
                "--rpc-url",
                rpc_url,
                "--skip-verify-chain-id",
                "--native-token-price",
                "1",
                "--txs-db-dir",
                &tmp_dir.to_string_lossy(),
                "--format",
                "json",
            ])
            .output()
            .expect("failed to execute CLI")
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

        let output = run_query(&rpc_url, &tmp_dir);

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

        let indexed: Vec<u64> = sqlx::query_scalar::<_, i64>(
            "SELECT block_number FROM indexed_blocks ORDER BY block_number",
        )
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

        assert_eq!(indexed, expected, "indexed_blocks mismatch");
        assert_eq!(tx_blocks, expected, "transaction blocks mismatch");
        assert_eq!(
            indexed, tx_blocks,
            "indexed_blocks vs transactions mismatch"
        );
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
    async fn test_query_reports_cached_blocks_on_repeat_run() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        let total_blocks = TO_BLOCK - FROM_BLOCK + 1;

        let first = run_query(&rpc_url, &tmp_dir);
        assert!(
            first.status.success(),
            "first query failed: {}",
            String::from_utf8_lossy(&first.stderr),
        );
        let first_json: QueryResponse = serde_json::from_slice(&first.stdout)?;
        assert_eq!(first_json.cached_blocks, 0, "first run cached_blocks");
        assert_eq!(first_json.new_blocks, total_blocks, "first run new_blocks");

        let second = run_query(&rpc_url, &tmp_dir);
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
