use std::path::{Path, PathBuf};

use eyre::Result;
use sqlx::{
    Sqlite, SqlitePool,
    migrate::{MigrateDatabase, Migrator},
};
use tracing::info;

use super::shared_init::config_path;

// Signatures database (events, methods, chains). Shipped as a prebuilt file
// downloaded from the CDN, so its migrations live in `migrations/sigs`.
static SIGS_MIGRATOR: Migrator = sqlx::migrate!("migrations/sigs");
pub const SIGS_DB_SCHEMA_VERSION: u64 = 4;

// Transactions database (locally built tx store). Its migrations live in
// `migrations/txs` and are applied independently from the signatures database.
static TXS_MIGRATOR: Migrator = sqlx::migrate!("migrations/txs");
pub const TXS_DB_SCHEMA_VERSION: u64 = 1;

pub async fn init_sigs_db(db_url: Option<String>) -> Result<()> {
    let db_url = db_url.unwrap_or(default_sigs_db_path().to_string_lossy().into_owned());
    create_db_if_missing(&db_url, &SIGS_MIGRATOR).await
}

pub async fn init_txs_db(db_url: Option<String>) -> Result<()> {
    let db_url = db_url.unwrap_or(default_txs_db_path().to_string_lossy().into_owned());
    create_db_if_missing(&db_url, &TXS_MIGRATOR).await
}

async fn create_db_if_missing(db_url: &str, migrator: &Migrator) -> Result<()> {
    if Sqlite::database_exists(db_url).await.unwrap_or(false) {
        info!("Database {} already exists", db_url);
        return Ok(());
    }

    info!("Creating database {}", db_url);
    if let Some(parent) = Path::new(db_url).parent() {
        std::fs::create_dir_all(parent)?;
    }

    match Sqlite::create_database(db_url).await {
        Ok(_) => {
            info!("Create {} db success", db_url);

            let db = SqlitePool::connect(db_url).await?;
            match migrator.run(&db).await {
                Ok(_) => info!("Migrations run successfully"),
                Err(error) => panic!("Failed to run migrations: {error}"),
            }
        }
        Err(error) => panic!("error: {error}"),
    }

    Ok(())
}

pub async fn sigs_conn(db_url: Option<String>) -> Result<SqlitePool> {
    let db_url = db_url.unwrap_or(default_sigs_db_path().to_string_lossy().into_owned());
    connect(&db_url).await
}

pub async fn txs_conn(db_url: Option<String>) -> Result<SqlitePool> {
    let db_url = db_url.unwrap_or(default_txs_db_path().to_string_lossy().into_owned());
    connect(&db_url).await
}

async fn connect(db_url: &str) -> Result<SqlitePool> {
    match SqlitePool::connect(db_url).await {
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

pub fn sigs_db_file_name(schema_version: u64) -> String {
    format!("mevlog-sqlite-v{schema_version}.db")
}

pub fn default_sigs_db_path() -> PathBuf {
    config_path().join(sigs_db_file_name(SIGS_DB_SCHEMA_VERSION))
}

pub fn txs_db_file_name(schema_version: u64) -> String {
    format!("mevlog-txs-v{schema_version}.db")
}

pub fn default_txs_db_path() -> PathBuf {
    config_path().join(txs_db_file_name(TXS_DB_SCHEMA_VERSION))
}
