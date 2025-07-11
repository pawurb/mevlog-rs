use std::{
    fs::File,
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

        // Seed chains first
        self.seed_chains(&conn).await?;

        // Then seed signatures if SEED_FILE_URL is set
        if let Ok(file_path) = std::env::var("SEED_FILE_URL") {
            self.seed_signatures(&conn, file_path).await?;
        } else {
            tracing::info!("SEED_FILE_URL not set, skipping signature seeding");
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

        // Check if chain already exists
        if DbChain::exists(chain_data.chain_id as i64, conn).await? {
            return Ok(false);
        }

        // Get the first explorer URL if available
        let explorer_url = chain_data.explorers.first().map(|e| e.url.clone());

        let db_chain = DbChain {
            id: chain_data.chain_id as i64,
            name: chain_data.name,
            explorer_url,
            currency_symbol: chain_data.native_currency.symbol,
            chainlink_oracle: None, // Not available in the source data
        };

        db_chain.save(conn).await?;
        Ok(true)
    }

    async fn seed_signatures(&self, conn: &sqlx::SqlitePool, file_path: String) -> Result<()> {
        tracing::info!("Seeding signatures");

        let file1 = File::open(file_path.clone())?;
        let file2 = File::open(file_path)?;
        let count_reader = BufReader::new(file1);
        let reader = BufReader::new(file2);

        let total_lines = count_reader.lines().count();

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
        }

        Ok(())
    }
}
