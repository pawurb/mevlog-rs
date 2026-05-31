use std::collections::HashMap;

use eyre::Result;
use sqlx::Row;
use tokio::sync::RwLock;

pub mod db_chain;
pub mod db_event;
pub mod db_method;

async fn find_signature(
    table: &str,
    column: &str,
    cache: &RwLock<HashMap<String, Option<String>>>,
    signature_hash: &str,
    conn: &sqlx::SqlitePool,
) -> Result<Option<String>> {
    let key = normalize_key(signature_hash);

    if let Some(cached) = cache.read().await.get(&key).cloned() {
        return Ok(cached);
    }

    let signature_hash_bytes = hex::decode(&key).expect("Invalid hex");

    let query = format!("SELECT signature FROM {table} WHERE {column} = ? LIMIT 1");
    let result = sqlx::query(&query)
        .bind(signature_hash_bytes)
        .fetch_optional(conn)
        .await?;

    let found: Option<String> = result.map(|row| row.get(0));

    cache.write().await.insert(key, found.clone());

    Ok(found)
}

fn normalize_key(signature_hash: &str) -> String {
    signature_hash.trim_start_matches("0x").to_ascii_lowercase()
}
