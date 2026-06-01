use std::path::{Path, PathBuf};

use eyre::Result;
use sqlx::{
    Sqlite, SqlitePool,
    migrate::{MigrateDatabase, Migrator},
    sqlite::SqliteConnectOptions,
};
use tracing::info;

fn resolve_url(db_url: Option<String>, default_path: PathBuf) -> String {
    db_url.unwrap_or_else(|| default_path.to_string_lossy().into_owned())
}

pub async fn init_db(
    db_url: Option<String>,
    default_path: PathBuf,
    migrator: &Migrator,
) -> Result<()> {
    let db_url = resolve_url(db_url, default_path);

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
            info!("Create {} db success", db_url);

            let db = SqlitePool::connect(&db_url).await?;
            match migrator.run(&db).await {
                Ok(_) => info!("Migrations run successfully"),
                Err(error) => panic!("Failed to run migrations: {error}"),
            }
        }
        Err(error) => panic!("error: {error}"),
    }

    Ok(())
}

pub async fn conn(
    db_url: Option<String>,
    default_path: PathBuf,
    read_only: bool,
) -> Result<SqlitePool> {
    let db_url = resolve_url(db_url, default_path);
    // Accept both `sqlite://<path>` URLs and bare filesystem paths.
    let filename = db_url
        .strip_prefix("sqlite://")
        .or_else(|| db_url.strip_prefix("sqlite:"))
        .unwrap_or(&db_url);

    let opts = SqliteConnectOptions::new()
        .filename(filename)
        .read_only(read_only)
        .create_if_missing(false);

    match SqlitePool::connect_with(opts).await {
        Ok(sqlite) => Ok(sqlite),
        Err(error) => eyre::bail!("Error connecting to db: {}", error),
    }
}

pub async fn truncate_wal(conn: &SqlitePool) -> Result<()> {
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(conn)
        .await?;

    Ok(())
}
