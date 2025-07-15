use std::{
    cmp::min,
    fs::{self, File},
    io::{Read, Write},
};

use eyre::{eyre, OptionExt, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use ruzstd::decoding::StreamingDecoder;
use sqlx::{Connection, SqliteConnection};

use crate::misc::database::{db_file_name, default_db_path, DB_SCHEMA_VERSION};

pub const PROGRESS_CHARS: &str = "█▓▒░─";

pub fn db_file_exists() -> bool {
    default_db_path().exists()
}

pub async fn remove_db_files() -> Result<()> {
    let path = default_db_path();

    if path.exists() {
        let str_path = default_db_path().to_string_lossy().into_owned();
        let pattern = format!("{str_path}*");
        for entry in glob::glob(&pattern).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => match fs::remove_file(&path) {
                    Ok(_) => {
                        println!("Removed database file at: {}", path.display());
                    }
                    Err(e) => {
                        eprintln!("Failed to remove file {path:?}: {e}");
                    }
                },
                Err(e) => eprintln!("Error reading glob entry: {e}"),
            }
        }
    }
    Ok(())
}

pub async fn download_db_file() -> Result<()> {
    let url = db_file_url();
    let client = Client::new();
    let db_path = default_db_path().to_string_lossy().into_owned();

    let zst_path = format!("{db_path}.zst");

    let res = client
        .get(url.clone())
        .send()
        .await
        .map_err(|e| eyre!("Failed to GET from '{}': {}", url, e))?;
    let compressed_size = res
        .content_length()
        .ok_or_eyre("Failed to get content length")?;
    let uncompressed_size = res
        .headers()
        .get("x-amz-meta-uncompressed-size")
        .expect("Failed to get uncompressed size header")
        .to_str()
        .expect("Failed to convert uncompressed size header to string")
        .parse::<u64>()
        .expect("Invalid uncompressed size header");

    let pb = ProgressBar::new(compressed_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars(PROGRESS_CHARS));

    pb.set_message(format!("Downloading signatures database to: {zst_path}"));

    let mut zst_file =
        File::create(zst_path.clone()).map_err(|e| eyre!("Failed to create file: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| eyre!("Error while downloading file: {}", e))?;
        zst_file
            .write_all(&chunk)
            .map_err(|e| eyre!("Error while writing to file: {}", e))?;
        let new = min(downloaded + (chunk.len() as u64), compressed_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message("Download complete");

    let zst_file = File::open(zst_path.clone()).map_err(|e| eyre!("Failed to open file: {}", e))?;
    let mut db_file = File::create(db_path).map_err(|e| eyre!("Failed to create file: {}", e))?;

    let pb = ProgressBar::new(uncompressed_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars(PROGRESS_CHARS));
    pb.set_message("Decompressing database file".to_string());

    let mut zst_decoder = StreamingDecoder::new(&zst_file)
        .map_err(|e| eyre!("Failed to create zstd decoder: {}", e))?;
    let mut buffer = [0u8; 8192];
    let mut decompressed_size = 0;
    loop {
        let bytes_read = match zst_decoder.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => return Err(eyre!("Error during extraction: {}", e)),
        };
        if bytes_read == 0 {
            break;
        }
        db_file
            .write_all(&buffer[..bytes_read])
            .map_err(|e| eyre!("Failed writing decompressed data: {}", e))?;
        decompressed_size += bytes_read as u64;
        pb.set_position(decompressed_size);
    }

    pb.finish_with_message("Extraction complete");

    fs::remove_file(&zst_path).map_err(|e| eyre!("Failed to remove .zst file: {}", e))?;

    ensure_database_indexes().await?;

    Ok(())
}

async fn ensure_database_indexes() -> Result<()> {
    let db_path = default_db_path();
    let database_url = format!("sqlite:{}", db_path.to_string_lossy());

    let mut conn = SqliteConnection::connect(&database_url)
        .await
        .map_err(|e| eyre!("Failed to connect to database: {}", e))?;

    let indexes_to_check = [
        ("events_signature_hash_index", "events", "signature_hash"),
        ("methods_signature_hash_index", "methods", "signature_hash"),
    ];

    for (index_name, table_name, column_name) in indexes_to_check {
        let index_exists = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name=?",
        )
        .bind(index_name)
        .fetch_one(&mut conn)
        .await
        .map_err(|e| eyre!("Failed to check index existence: {}", e))?;

        if index_exists == 0 {
            let create_index_sql =
                format!("CREATE INDEX {index_name} ON {table_name} ({column_name})");

            println!("Creating index: {create_index_sql}");

            sqlx::query(&create_index_sql)
                .execute(&mut conn)
                .await
                .map_err(|e| eyre!("Failed to create index {}: {}", index_name, e))?;

            println!("Created index: {index_name}");
        }
    }

    conn.close()
        .await
        .map_err(|e| eyre!("Failed to close database connection: {}", e))?;

    Ok(())
}

pub async fn check_and_create_indexes() -> Result<()> {
    if !db_file_exists() {
        eyre::bail!("Database file does not exist")
    }

    ensure_database_indexes().await
}

fn db_file_url() -> String {
    format!(
        "https://d39my35jed0oxi.cloudfront.net/{}.zst",
        db_file_name(DB_SCHEMA_VERSION)
    )
}
