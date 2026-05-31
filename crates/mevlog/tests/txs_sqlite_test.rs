#[cfg(test)]
pub mod tests {
    use std::{fs, path::PathBuf, process::Command};

    use eyre::Result;
    use mevlog::db::txs;
    use uuid::Uuid;

    const CHAIN_ID: u64 = 1;
    const FROM_BLOCK: u64 = 25215353;
    const TO_BLOCK: u64 = 25215357;

    fn fixtures_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cryo/ethereum")
    }

    fn cryo_cache_dir() -> PathBuf {
        home::home_dir()
            .expect("home dir")
            .join(".mevlog/.cryo-cache/ethereum")
    }

    /// Copies the fixture parquet files into the real cryo cache location so
    /// `query` finds full coverage and never hits cryo/RPC for block data.
    fn sync_fixtures_to_cache() {
        let cache = cryo_cache_dir();
        fs::create_dir_all(&cache).expect("create cryo cache dir");

        for entry in fs::read_dir(fixtures_dir()).expect("read fixtures dir") {
            let path = entry.expect("fixture entry").path();
            let name = path.file_name().expect("fixture file name");
            fs::copy(&path, cache.join(name)).expect("copy fixture into cache");
        }
    }

    #[tokio::test]
    async fn test_query_indexes_txs_into_sqlite() -> Result<()> {
        let rpc_url = std::env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set");

        sync_fixtures_to_cache();

        let tmp_dir = std::env::temp_dir().join(format!("mevlog-sqlite-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&tmp_dir)?;

        // Start from a clean slate: remove any pre-existing txs DB (and its WAL
        // sidecar files) so the test never indexes into stale data.
        let db_path = tmp_dir.join(txs::db_file_name(txs::SCHEMA_VERSION, CHAIN_ID));
        for suffix in ["", "-wal", "-shm"] {
            let p = PathBuf::from(format!("{}{suffix}", db_path.to_string_lossy()));
            fs::remove_file(&p).ok();
        }

        let output = Command::new("cargo")
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
                &rpc_url,
                "--skip-verify-chain-id",
                "--native-token-price",
                "1",
                "--txs-db-dir",
                &tmp_dir.to_string_lossy(),
                "--format",
                "json",
            ])
            .output()
            .expect("failed to execute CLI");

        assert!(
            output.status.success(),
            "query failed: stdout={}, stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let conn = txs::conn(Some(db_path.to_string_lossy().into_owned()), CHAIN_ID).await?;

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

        assert_eq!(indexed, expected, "indexed_blocks mismatch");
        assert_eq!(tx_blocks, expected, "transaction blocks mismatch");
        assert_eq!(
            indexed, tx_blocks,
            "indexed_blocks vs transactions mismatch"
        );

        fs::remove_dir_all(&tmp_dir).ok();
        Ok(())
    }
}
