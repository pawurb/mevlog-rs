pub mod custom_tables;
pub mod display_sql;
pub mod indexing;
pub mod info;
pub mod models;
pub mod purge;
pub mod raw_query;

use std::path::PathBuf;

use eyre::Result;
use sqlx::{SqlitePool, migrate::Migrator};

use crate::{db::shared, misc::shared_init::config_path};

// Transactions database (locally built tx store). Its migrations live in
// `migrations/txs` and are applied independently from the signatures database.
static MIGRATOR: Migrator = sqlx::migrate!("migrations/txs");
pub const SCHEMA_VERSION: u64 = 1;

pub(crate) async fn init_db(db_url: Option<String>, chain_id: u64) -> Result<()> {
    shared::init_db(db_url, default_db_path(chain_id), &MIGRATOR).await
}

pub async fn conn(db_url: Option<String>, chain_id: u64, read_only: bool) -> Result<SqlitePool> {
    shared::conn(db_url, default_db_path(chain_id), read_only).await
}

pub fn db_file_name(schema_version: u64, chain_id: u64) -> String {
    format!("mevlog-txs-v{schema_version}-{chain_id}.db")
}

/// Resolves the txs DB file path: `txs_db_dir` override joined with the
/// canonical file name, or the default location under `~/.mevlog`.
pub fn resolve_db_path(txs_db_dir: Option<&str>, chain_id: u64) -> PathBuf {
    match txs_db_dir {
        Some(dir) => PathBuf::from(dir).join(db_file_name(SCHEMA_VERSION, chain_id)),
        None => default_db_path(chain_id),
    }
}

pub(crate) fn default_db_path(chain_id: u64) -> PathBuf {
    config_path().join(db_file_name(SCHEMA_VERSION, chain_id))
}
