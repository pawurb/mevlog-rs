pub mod actions;
pub mod models;

use std::path::PathBuf;

use eyre::Result;
use sqlx::{SqlitePool, migrate::Migrator};

use crate::{db::shared, misc::shared_init::config_path};

// Signatures database (signatures, chains). Shipped as a prebuilt file
// downloaded from the CDN, so its migrations live in `migrations/sigs`.
static MIGRATOR: Migrator = sqlx::migrate!("migrations/sigs");
pub const SCHEMA_VERSION: u64 = 5;

pub async fn init_db(db_url: Option<String>) -> Result<()> {
    shared::init_db(db_url, default_db_path(), &MIGRATOR).await
}

pub async fn conn(db_url: Option<String>) -> Result<SqlitePool> {
    shared::conn(db_url, default_db_path()).await
}

pub fn db_file_name(schema_version: u64) -> String {
    format!("mevlog-sqlite-v{schema_version}.db")
}

pub fn default_db_path() -> PathBuf {
    config_path().join(db_file_name(SCHEMA_VERSION))
}
