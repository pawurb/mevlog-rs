use std::{
    io::{BufRead, BufReader},
    path::Path,
};

use clap::Parser;
use eyre::Result;
use mevlog::{
    misc::database::{init_sqlite_db, sqlite_conn, sqlite_truncate_wal},
    models::{db_chain::DbChain, db_event::DBEvent, db_method::DBMethod},
};
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Parser)]
pub struct SeedDBArgs {}

#[derive(Debug, Deserialize, Serialize)]
struct ChainData {
    name: String,
    #[serde(rename = "chainId")]
    chain_id: u64,
    #[serde(rename = "nativeCurrency")]
    native_currency: NativeCurrency,
    #[serde(default)]
    explorers: Vec<Explorer>,
}

#[derive(Debug, Deserialize, Serialize)]
struct NativeCurrency {
    name: String,
    symbol: String,
    decimals: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct Explorer {
    name: String,
    url: String,
    #[serde(default)]
    standard: Option<String>,
}

impl SeedDBArgs {
    #[allow(dead_code)]
    pub async fn run(&self) -> Result<()> {
        println!("Seeding db");
        init_sqlite_db(None).await?;
        let conn = sqlite_conn(None).await?;

        tracing::info!("Seeding database");

        self.seed_chains(&conn).await?;

        if std::env::var("SEED_SIGNATURES").unwrap_or_default() == "true" {
            self.seed_signatures(&conn).await?;
        } else {
            tracing::info!("SEED_SIGNATURES not set to true, skipping signature seeding");
        }

        info!("Truncating WAL");
        sqlite_truncate_wal(&conn).await?;

        info!("Finished seeding database");

        Ok(())
    }

    async fn seed_chains(&self, conn: &sqlx::SqlitePool) -> Result<()> {
        tracing::info!("Seeding chains");

        // Local copy of https://github.com/ethereum-lists/chains
        let chains_dir = "../chains/_data/chains";
        if !Path::new(chains_dir).exists() {
            tracing::warn!("Chains directory not found: {}", chains_dir);
            return Ok(());
        }

        let pattern = format!("{chains_dir}/*.json");
        let paths = glob::glob(&pattern)?;

        let mut total_processed = 0;
        let mut total_success = 0;

        for entry in paths {
            match entry {
                Ok(path) => {
                    total_processed += 1;

                    if total_processed % 500 == 0 {
                        tracing::info!("Processed {} chain files", total_processed);
                    }

                    match self.process_chain_file(&path, conn).await {
                        Ok(true) => total_success += 1,
                        Ok(false) => {
                            tracing::debug!("Skipped chain file: {}", path.display());
                        }
                        Err(e) => {
                            tracing::warn!("Error processing chain file {}: {}", path.display(), e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Error reading chain file path: {}", e);
                }
            }
        }

        tracing::info!(
            "Chains seeding completed. Processed: {}, Success: {}",
            total_processed,
            total_success
        );
        Ok(())
    }

    async fn process_chain_file(&self, path: &Path, conn: &sqlx::SqlitePool) -> Result<bool> {
        let file_content = std::fs::read_to_string(path)?;
        let chain_data: ChainData = serde_json::from_str(&file_content)?;

        if DbChain::exists(chain_data.chain_id as i64, conn).await? {
            return Ok(false);
        }

        let explorer_url = chain_data.explorers.first().map(|e| e.url.clone());

        let db_chain = DbChain {
            id: chain_data.chain_id as i64,
            name: chain_data.name,
            explorer_url,
            currency_symbol: chain_data.native_currency.symbol,
            chainlink_oracle: None,
            uniswap_v2_pool: None,
        };

        db_chain.save(conn).await?;
        Ok(true)
    }

    async fn seed_signatures(&self, conn: &sqlx::SqlitePool) -> Result<()> {
        tracing::info!("Seeding signatures from OpenChain API");

        let signatures_file = self.get_or_download_signatures_file().await?;

        tracing::info!("Processing signature data from local file");

        // Count total lines first
        let file_for_count = std::fs::File::open(&signatures_file)?;
        let reader_for_count = BufReader::new(file_for_count);
        let total_lines = reader_for_count.lines().count();

        tracing::info!("Total signature lines to process: {}", total_lines);

        let file = std::fs::File::open(&signatures_file)?;
        let reader = BufReader::new(file);

        let mut line_count = 0;
        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            let Some((signature_hash, signature)) = line.split_once(',') else {
                continue;
            };

            if i.is_multiple_of(10000) {
                tracing::info!("Processing signature line {}/{}", i, total_lines);
            }

            if signature_hash.len() == 10 {
                let new_method = DBMethod {
                    signature: signature.to_string(),
                    signature_hash: signature_hash.to_string(),
                };

                let _ = new_method.save(conn).await;
            }

            if signature_hash.len() == 66 {
                let new_event = DBEvent {
                    signature: signature.to_string(),
                    signature_hash: signature_hash.to_string(),
                };

                let _ = new_event.save(conn).await;
            }

            line_count += 1;
        }

        tracing::info!("Processed {} signature lines", line_count);

        Ok(())
    }

    async fn get_or_download_signatures_file(&self) -> Result<String> {
        let today = chrono::Utc::now().format("%Y-%m-%d");
        let signatures_file = format!("/tmp/signatures_{today}.csv");

        if Path::new(&signatures_file).exists() {
            tracing::info!("Using existing signatures file: {}", signatures_file);
            return Ok(signatures_file);
        }

        let url = "https://api.openchain.xyz/signature-database/v1/export";
        tracing::info!("Downloading signatures database from: {}", url);

        let response = reqwest::get(url).await?;
        let mut file = std::fs::File::create(&signatures_file)?;
        let mut stream = response.bytes_stream();

        use std::io::Write;

        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk)?;
        }

        file.flush()?;
        tracing::info!("Downloaded signature database to: {}", signatures_file);

        Ok(signatures_file)
    }
}
