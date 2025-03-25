use std::{
    cmp::min,
    fs::{self, File},
    io::{Read, Write},
};

use eyre::{eyre, OptionExt, Result};
use flate2::read::GzDecoder;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;

use crate::misc::database::default_db_path;

const DB_FILE_URL: &str = "https://d39my35jed0oxi.cloudfront.net/signatures-sqlite-v3.db.gz";
pub const PROGRESS_CHARS: &str = "█▓▒░─";

pub fn db_file_exists() -> bool {
    default_db_path().exists()
}

pub async fn remove_db_files() -> Result<()> {
    let path = default_db_path();

    if path.exists() {
        let str_path = default_db_path().to_string_lossy().into_owned();
        let pattern = format!("{}*", str_path);
        for entry in glob::glob(&pattern).expect("Failed to read glob pattern") {
            match entry {
                Ok(path) => match fs::remove_file(&path) {
                    Ok(_) => {
                        println!("Removed database file at: {}", path.display());
                    }
                    Err(e) => {
                        eprintln!("Failed to remove file {:?}: {}", path, e);
                    }
                },
                Err(e) => eprintln!("Error reading glob entry: {}", e),
            }
        }
    }
    Ok(())
}

pub async fn download_db_file() -> Result<()> {
    let url = DB_FILE_URL;
    let client = Client::new();
    let db_path = default_db_path().to_string_lossy().into_owned();

    let gz_path = format!("{}.gz", db_path);

    let res = client
        .get(url)
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

    pb.set_message(format!("Downloading signatures database to: {}", gz_path));

    let mut gz_file =
        File::create(gz_path.clone()).map_err(|e| eyre!("Failed to create file: {}", e))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.map_err(|e| eyre!("Error while downloading file: {}", e))?;
        gz_file
            .write_all(&chunk)
            .map_err(|e| eyre!("Error while writing to file: {}", e))?;
        let new = min(downloaded + (chunk.len() as u64), compressed_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message("Download complete");

    let gz_file = File::open(gz_path.clone()).map_err(|e| eyre!("Failed to open file: {}", e))?;
    let mut db_file = File::create(db_path).map_err(|e| eyre!("Failed to create file: {}", e))?;

    let pb = ProgressBar::new(uncompressed_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars(PROGRESS_CHARS));
    pb.set_message("Unzipping database file".to_string());

    let mut gz_decoder = GzDecoder::new(gz_file);
    let mut buffer = [0u8; 8192];
    let mut decompressed_size = 0;
    loop {
        let bytes_read = gz_decoder
            .read(&mut buffer)
            .map_err(|e| eyre!("Error during extraction: {}", e))?;
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

    fs::remove_file(&gz_path).map_err(|e| eyre!("Failed to remove .gz file: {}", e))?;

    Ok(())
}
