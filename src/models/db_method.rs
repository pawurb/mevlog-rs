use eyre::Result;
use sqlx::Row;

#[allow(dead_code)]
#[derive(Debug)]
pub struct DBMethod {
    pub signature_hash: String,
    pub signature: String,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for DBMethod {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(DBMethod {
            signature_hash: row.get(0),
            signature: row.try_get(1)?,
        })
    }
}

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

    pub async fn find_by_hash(
        signature_hash: &str,
        conn: &sqlx::SqlitePool,
    ) -> Result<Option<String>> {
        let signature_hash = signature_hash[2..].to_string();
        let result = sqlx::query(
            r#"
            SELECT signature FROM methods WHERE signature_hash = ? LIMIT 1
            "#,
        )
        .bind(signature_hash.as_bytes().to_vec())
        .fetch_optional(conn)
        .await?;

        match result {
            Some(row) => Ok(Some(row.get(0))),
            None => Ok(None),
        }
    }

    pub async fn save(&self, conn: &sqlx::SqlitePool) -> Result<()> {
        let signature_hash_bytes: Vec<u8> = self.signature_hash.as_bytes().to_vec();
        let signature_hash_bytes: Vec<u8> = signature_hash_bytes.iter().skip(2).cloned().collect();

        sqlx::query(
            r#"
            INSERT INTO methods (signature_hash, signature)
            VALUES (?, ?)
            "#,
        )
        .bind(signature_hash_bytes)
        .bind(&self.signature)
        .execute(conn)
        .await?;

        Ok(())
    }
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
}
