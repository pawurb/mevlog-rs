use std::collections::HashMap;

use eyre::Result;
use sqlx::Row;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct DBMethod {
    pub signature_hash: String,
    pub signature: String,
}

static METHOD_SIG_MEMORY_CACHE: std::sync::LazyLock<RwLock<HashMap<String, Option<String>>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

impl DBMethod {
    pub async fn exists(signature: &str, conn: &sqlx::SqlitePool) -> Result<bool> {
        let exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM methods WHERE signature = ?)")
            .bind(signature)
            .fetch_one(conn)
            .await?
            .get::<bool, _>(0);

        Ok(exists)
    }

    pub async fn count(conn: &sqlx::SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM methods")
            .fetch_one(conn)
            .await?
            .get::<i64, _>(0);

        Ok(count)
    }

    #[cfg_attr(feature = "hotpath", hotpath::measure)]
    pub async fn find_by_hash(
        signature_hash: &str,
        conn: &sqlx::SqlitePool,
    ) -> Result<Option<String>> {
        let key = normalize_key(signature_hash);

        if let Some(cached) = METHOD_SIG_MEMORY_CACHE.read().await.get(&key).cloned() {
            return Ok(cached);
        }

        let signature_hash_bytes = hex::decode(&key).expect("Invalid hex");

        let result = sqlx::query(
            r#"
            SELECT signature FROM methods WHERE signature_hash = ? LIMIT 1
            "#,
        )
        .bind(signature_hash_bytes)
        .fetch_optional(conn)
        .await?;

        let found: Option<String> = result.map(|row| row.get(0));

        METHOD_SIG_MEMORY_CACHE
            .write()
            .await
            .insert(key, found.clone());

        Ok(found)
    }

    pub async fn save<'c, E>(&self, executor: E) -> Result<()>
    where
        E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
    {
        let key = normalize_key(&self.signature_hash);
        let signature_hash_bytes = hex::decode(&key).expect("Invalid hex");

        sqlx::query(
            r#"
            INSERT INTO methods (signature_hash, signature)
            VALUES (?, ?)
            "#,
        )
        .bind(signature_hash_bytes)
        .bind(&self.signature)
        .execute(executor)
        .await?;

        Ok(())
    }
}

fn normalize_key(signature_hash: &str) -> String {
    signature_hash.trim_start_matches("0x").to_ascii_lowercase()
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::models::db_event::test::setup_test_db;

    #[tokio::test]
    async fn create_and_get_method() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let new_method = DBMethod {
            signature_hash: "0x022c0d9f".to_string(),
            signature: "swap(uint256,uint256,address,bytes)".to_string(),
        };

        new_method.save(&conn).await?;

        let exists = DBMethod::exists(&new_method.signature, &conn).await?;

        assert_eq!(DBMethod::count(&conn).await?, 1);

        assert!(exists);

        assert_eq!(DBMethod::count(&conn).await?, 1);

        let other_method = DBMethod {
            signature_hash: "0x3ccfd60b".to_string(),
            signature: "withdraw()".to_string(),
        };

        other_method.save(&conn).await?;

        let signature = DBMethod::find_by_hash("0x3ccfd60b", &conn).await?;

        assert_eq!(signature.unwrap(), "withdraw()");

        Ok(())
    }

    #[tokio::test]
    async fn save_with_transaction() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut tx = conn.begin().await?;

        let method1 = DBMethod {
            signature_hash: "0x022c0d9f".to_string(),
            signature: "swap(uint256,uint256,address,bytes)".to_string(),
        };

        let method2 = DBMethod {
            signature_hash: "0x3ccfd60b".to_string(),
            signature: "withdraw()".to_string(),
        };

        method1.save(&mut *tx).await?;
        method2.save(&mut *tx).await?;

        // Before commit, count should be 0 from outside transaction
        assert_eq!(DBMethod::count(&conn).await?, 0);

        tx.commit().await?;

        // After commit, both methods should be saved
        assert_eq!(DBMethod::count(&conn).await?, 2);

        let signature1 = DBMethod::find_by_hash("0x022c0d9f", &conn).await?;
        let signature2 = DBMethod::find_by_hash("0x3ccfd60b", &conn).await?;

        assert_eq!(signature1.unwrap(), "swap(uint256,uint256,address,bytes)");
        assert_eq!(signature2.unwrap(), "withdraw()");

        Ok(())
    }
}
