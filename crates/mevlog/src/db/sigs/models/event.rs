use std::collections::HashMap;

use eyre::Result;
use sqlx::Row;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct Event {
    pub signature_hash_32: Vec<u8>,
    pub signature: String,
}

static TOPIC_SIG_MEMORY_CACHE: std::sync::LazyLock<
    hotpath::wrap::tokio::sync::RwLock<HashMap<String, Option<String>>>,
> = std::sync::LazyLock::new(|| {
    hotpath::rw_lock!(RwLock::new(HashMap::new()), label = "topic_sig_cache")
});

#[hotpath::measure_all(future = true)]
impl Event {
    #[allow(dead_code)] // used in tests
    pub(crate) async fn count(conn: &sqlx::SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM events")
            .fetch_one(conn)
            .await?
            .get::<i64, _>(0);

        Ok(count)
    }

    pub(crate) async fn find_by_topic(
        signature_hash: &str,
        conn: &sqlx::SqlitePool,
    ) -> Result<Option<String>> {
        let key = signature_hash.trim_start_matches("0x").to_ascii_lowercase();

        if let Some(cached) = TOPIC_SIG_MEMORY_CACHE.read().await.get(&key).cloned() {
            return Ok(cached);
        }

        let signature_hash_bytes = hex::decode(&key).expect("Invalid hex");

        let found: Option<String> =
            sqlx::query_scalar("SELECT signature FROM events WHERE signature_hash_32 = ? LIMIT 1")
                .bind(signature_hash_bytes)
                .fetch_optional(conn)
                .await?;

        TOPIC_SIG_MEMORY_CACHE
            .write()
            .await
            .insert(key, found.clone());

        Ok(found)
    }

    pub async fn save<'c, E>(&self, executor: E) -> Result<()>
    where
        E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
    {
        sqlx::query(
            r#"
            INSERT INTO events (signature_hash_32, signature)
            VALUES (?, ?)
            "#,
        )
        .bind(&self.signature_hash_32)
        .bind(&self.signature)
        .execute(executor)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use std::fs;

    use sqlx::sqlite::SqlitePool;
    use uuid::Uuid;

    use super::*;
    use crate::db::sigs::{conn, init_db};

    pub(crate) async fn setup_test_db() -> (SqlitePool, SqliteCleaner) {
        let uuid = Uuid::new_v4();
        let db_path = format!("/tmp/{uuid}-mevlog-test.db");
        let db_url = format!("sqlite://{db_path}");

        if fs::remove_file(&db_url).is_ok() {
            println!("DB {} removed", db_url);
        }

        init_db(Some(db_url.clone()))
            .await
            .expect("Failed to init db");

        let cleaner = SqliteCleaner {
            db_uuid: uuid.to_string(),
        };

        (
            conn(Some(db_url)).await.expect("Failed to connect to db"),
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

    fn event(hash_32_hex: &str, signature: &str) -> Event {
        Event {
            signature_hash_32: hex::decode(hash_32_hex).unwrap(),
            signature: signature.to_string(),
        }
    }

    #[tokio::test]
    async fn save_and_find_by_topic() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let transfer = event(
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            "Transfer(address,address,uint256)",
        );
        transfer.save(&conn).await?;

        assert_eq!(Event::count(&conn).await?, 1);

        let found = Event::find_by_topic(
            "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
            &conn,
        )
        .await?;
        assert_eq!(found.unwrap(), "Transfer(address,address,uint256)");

        Ok(())
    }

    #[tokio::test]
    async fn save_with_transaction() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut tx = conn.begin().await?;

        let approval = event(
            "8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925",
            "Approval(address,address,uint256)",
        );
        approval.save(&mut *tx).await?;

        // Before commit, count is 0 from outside the transaction.
        assert_eq!(Event::count(&conn).await?, 0);

        tx.commit().await?;

        assert_eq!(Event::count(&conn).await?, 1);

        let found = Event::find_by_topic(
            "0x8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925",
            &conn,
        )
        .await?;
        assert_eq!(found.unwrap(), "Approval(address,address,uint256)");

        Ok(())
    }
}
