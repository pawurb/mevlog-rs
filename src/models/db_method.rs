use eyre::Result;
use sqlx::Row;

#[allow(dead_code)]
#[derive(Debug)]
pub struct DBMethod {
    id: i64,
    signature_hash: String,
    signature: String,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for DBMethod {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(DBMethod {
            id: row.get(0),
            signature_hash: row.get(1),
            signature: row.try_get(2)?,
        })
    }
}

#[derive(Debug)]
pub struct NewMethod {
    pub signature_hash: String,
    pub signature: String,
}

impl DBMethod {
    pub async fn exists(id: &i64, conn: &sqlx::SqlitePool) -> Result<bool> {
        let exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM methods WHERE id = ?)")
            .bind(id)
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
        let result = sqlx::query(
            r#"
            SELECT signature FROM methods WHERE signature_hash = ?
            "#,
        )
        .bind(signature_hash)
        .fetch_optional(conn)
        .await?;

        match result {
            Some(row) => Ok(Some(row.get(0))),
            None => Ok(None),
        }
    }
}

impl NewMethod {
    pub async fn save(&self, conn: &sqlx::SqlitePool) -> Result<DBMethod> {
        let event: DBMethod = sqlx::query_as(
            r#"
            INSERT INTO methods (signature_hash, signature)
            VALUES (?, ?)
            RETURNING *
            "#,
        )
        .bind(&self.signature_hash)
        .bind(&self.signature)
        .fetch_one(conn)
        .await?;

        Ok(event)
    }
}

#[cfg(test)]
pub mod test {
    use super::*;
    use crate::models::db_event::test::setup_test_db;

    #[tokio::test]
    async fn create_and_get_event() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let new_event = NewMethod {
            signature_hash: "0x022c0d9f".to_string(),
            signature: "swap(uint256,uint256,address,bytes)".to_string(),
        };

        let event = new_event.save(&conn).await?;

        let exists = DBMethod::exists(&event.id, &conn).await?;

        assert_eq!(DBMethod::count(&conn).await?, 1);

        assert!(exists);

        assert_eq!(DBMethod::count(&conn).await?, 1);

        let other_event = NewMethod {
            signature_hash: "0x3ccfd60b".to_string(),
            signature: "withdraw()".to_string(),
        };

        other_event.save(&conn).await?;

        let signature = DBMethod::find_by_hash("0x3ccfd60b", &conn).await?;

        assert!(signature.is_some());

        Ok(())
    }
}
