use std::collections::HashMap;

use eyre::Result;
use sqlx::Row;
use tokio::sync::RwLock;

use super::find_signature;

#[derive(Debug)]
pub struct DBMethod {
    pub signature_hash_4: Vec<u8>,
    pub signature: String,
}

static SELECTOR_SIG_MEMORY_CACHE: std::sync::LazyLock<RwLock<HashMap<String, Option<String>>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

#[hotpath::measure_all(future = true)]
impl DBMethod {
    pub async fn count(conn: &sqlx::SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM methods")
            .fetch_one(conn)
            .await?
            .get::<i64, _>(0);

        Ok(count)
    }

    pub async fn find_by_selector(
        signature_hash: &str,
        conn: &sqlx::SqlitePool,
    ) -> Result<Option<String>> {
        find_signature(
            "methods",
            "signature_hash_4",
            &SELECTOR_SIG_MEMORY_CACHE,
            signature_hash,
            conn,
        )
        .await
    }

    pub async fn save<'c, E>(&self, executor: E) -> Result<()>
    where
        E: sqlx::Executor<'c, Database = sqlx::Sqlite>,
    {
        sqlx::query(
            r#"
            INSERT INTO methods (signature_hash_4, signature)
            VALUES (?, ?)
            "#,
        )
        .bind(&self.signature_hash_4)
        .bind(&self.signature)
        .execute(executor)
        .await?;

        Ok(())
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::models::sigs::db_event::test::setup_test_db;

    fn method(hash_4_hex: &str, signature: &str) -> DBMethod {
        DBMethod {
            signature_hash_4: hex::decode(hash_4_hex).unwrap(),
            signature: signature.to_string(),
        }
    }

    #[tokio::test]
    async fn save_and_find_by_selector() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let swap = method("022c0d9f", "swap(uint256,uint256,address,bytes)");
        swap.save(&conn).await?;

        assert_eq!(DBMethod::count(&conn).await?, 1);

        let found = DBMethod::find_by_selector("0x022c0d9f", &conn).await?;
        assert_eq!(found.unwrap(), "swap(uint256,uint256,address,bytes)");

        let missing = DBMethod::find_by_selector("0xdeadbeef", &conn).await?;
        assert!(missing.is_none());

        Ok(())
    }

    #[tokio::test]
    async fn save_with_transaction() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let mut tx = conn.begin().await?;
        let transfer = method("a9059cbb", "transfer(address,uint256)");
        transfer.save(&mut *tx).await?;

        // Before commit, count is 0 from outside the transaction.
        assert_eq!(DBMethod::count(&conn).await?, 0);

        tx.commit().await?;

        assert_eq!(DBMethod::count(&conn).await?, 1);

        Ok(())
    }
}
