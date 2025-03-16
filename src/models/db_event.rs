use eyre::Result;
use sqlx::Row;

#[allow(dead_code)]
#[derive(Debug)]
pub struct DBEvent {
    pub signature_hash: String,
    pub signature: String,
}

impl sqlx::FromRow<'_, sqlx::sqlite::SqliteRow> for DBEvent {
    fn from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(DBEvent {
            signature_hash: row.get(0),
            signature: row.try_get(1)?,
        })
    }
}

impl DBEvent {
    pub async fn exists(signature: &str, conn: &sqlx::SqlitePool) -> Result<bool> {
        let exists = sqlx::query("SELECT EXISTS(SELECT 1 FROM events WHERE signature = ?)")
            .bind(signature)
            .fetch_one(conn)
            .await?
            .get::<bool, _>(0);

        Ok(exists)
    }

    pub async fn count(conn: &sqlx::SqlitePool) -> Result<i64> {
        let count = sqlx::query("SELECT COUNT(*) FROM events")
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
            SELECT signature FROM events WHERE signature_hash = ? LIMIT 1
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
            INSERT INTO events (signature_hash, signature)
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
    use std::{fs, sync::Arc};

    use crate::misc::database::{init_sqlite_db, sqlite_conn};

    use super::*;
    use sqlx::sqlite::SqlitePool;
    use uuid::Uuid;

    pub async fn setup_test_db() -> (Arc<SqlitePool>, SqliteCleaner) {
        let uuid = Uuid::new_v4();
        let db_path = format!("/tmp/{}-mevlog-test.db", uuid);
        let db_url = format!("sqlite://{}", db_path);

        if fs::remove_file(&db_url).is_ok() {
            println!("DB {} removed", &db_url);
        }

        init_sqlite_db(Some(db_url.clone()))
            .await
            .expect("Failed to init db");

        let cleaner = SqliteCleaner {
            db_uuid: uuid.to_string(),
        };

        (
            sqlite_conn(Some(db_url))
                .await
                .expect("Failed to connect to db"),
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
                            eprintln!("Failed to remove file {:?}: {}", path, e);
                        }
                    }
                    Err(e) => eprintln!("Error reading glob entry: {}", e),
                }
            }
        }
    }

    #[tokio::test]
    async fn create_and_get_event() -> Result<()> {
        let (conn, _cl) = setup_test_db().await;

        let new_event = DBEvent {
            signature_hash: "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
                .to_string(),
            signature: "Transfer(address,address,uint256)".to_string(),
        };

        dbg!(&new_event);
        new_event.save(&conn).await?;

        let exists = DBEvent::exists(&new_event.signature, &conn).await?;

        assert_eq!(DBEvent::count(&conn).await?, 1);

        assert!(exists);

        assert_eq!(DBEvent::count(&conn).await?, 1);

        let other_event = DBEvent {
            signature_hash: "0x45cceb0b830632de1c7fbebdf472f48e739c65f12da600c969011fc84dc602dd"
                .to_string(),
            signature: "Sync(u256,uint256)".to_string(),
        };

        other_event.save(&conn).await?;

        let signature = DBEvent::find_by_hash(
            "0x45cceb0b830632de1c7fbebdf472f48e739c65f12da600c969011fc84dc602dd",
            &conn,
        )
        .await?;

        assert_eq!(signature.unwrap(), "Sync(u256,uint256)");

        Ok(())
    }
}
