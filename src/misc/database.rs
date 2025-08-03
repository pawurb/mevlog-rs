use std::path::{Path, PathBuf};

use eyre::Result;
use sqlx::{
    migrate::{MigrateDatabase, Migrator},
    Sqlite, SqlitePool,
};
use tracing::info;

use super::shared_init::config_path;

static MIGRATOR: Migrator = sqlx::migrate!();
pub const DB_SCHEMA_VERSION: u64 = 4;

pub async fn init_sqlite_db(db_url: Option<String>) -> Result<()> {
    let db_url = db_url.unwrap_or(default_db_path().to_string_lossy().into_owned());

    if Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        info!("Database {} already exists", db_url);
        return Ok(());
    }

    info!("Creating database {}", db_url);
    if let Some(parent) = Path::new(&db_url).parent() {
        std::fs::create_dir_all(parent)?;
    }

    match Sqlite::create_database(&db_url).await {
        Ok(_) => {
            info!("Create {} db success", &db_url);

            let db = SqlitePool::connect(&db_url).await?;
            match MIGRATOR.run(&db).await {
                Ok(_) => info!("Migrations run successfully"),
                Err(error) => panic!("Failed to run migrations: {error}"),
            }
        }
        Err(error) => panic!("error: {error}"),
    }

    Ok(())
}

pub async fn sqlite_conn(db_url: Option<String>) -> Result<SqlitePool> {
    let db_url = db_url.unwrap_or(default_db_path().to_string_lossy().into_owned());

    match SqlitePool::connect(&db_url).await {
        Ok(sqlite) => Ok(sqlite),
        Err(error) => eyre::bail!("Error connecting to db: {}", error),
    }
}

pub async fn sqlite_truncate_wal(conn: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(conn)
        .await?;

    Ok(())
}

pub fn db_file_name(schema_version: u64) -> String {
    format!("mevlog-sqlite-v{schema_version}.db")
}

pub fn default_db_path() -> PathBuf {
    config_path().join(db_file_name(DB_SCHEMA_VERSION))
}
