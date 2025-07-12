use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::Path,
};

use clap::Parser;
use eyre::Result;
use mevlog::{
    misc::database::{init_sqlite_db, sqlite_conn, sqlite_truncate_wal},
    models::{db_chain::DBChain, db_event::DBEvent, db_method::DBMethod},
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

        let price_oracles = get_price_oracles();

        for entry in paths {
            match entry {
                Ok(path) => {
                    total_processed += 1;

                    if total_processed % 500 == 0 {
                        tracing::info!("Processed {} chain files", total_processed);
                    }

                    match self.process_chain_file(&path, &price_oracles, conn).await {
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

    async fn process_chain_file(
        &self,
        path: &Path,
        price_oracles: &HashMap<u64, String>,
        conn: &sqlx::SqlitePool,
    ) -> Result<bool> {
        let file_content = std::fs::read_to_string(path)?;
        let chain_data: ChainData = serde_json::from_str(&file_content)?;

        if DBChain::exists(chain_data.chain_id as i64, conn).await? {
            return Ok(false);
        }

        let explorer_url = chain_data.explorers.first().map(|e| e.url.clone());

        let db_chain = DBChain {
            id: chain_data.chain_id as i64,
            name: chain_data.name,
            explorer_url,
            currency_symbol: chain_data.native_currency.symbol,
            chainlink_oracle: price_oracles.get(&chain_data.chain_id).cloned(),
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
        let mut batch_count = 0;
        let batch_size = 1000;

        // Start first transaction
        let mut tx = conn.begin().await?;

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            let Some((signature_hash, signature)) = line.split_once(',') else {
                continue;
            };

            if i.is_multiple_of(10000) {
                tracing::info!("Processing signature line {}/{}", i, total_lines);
            }

            if signature_hash.len() == 10 {
                let method = DBMethod {
                    signature_hash: signature_hash.to_string(),
                    signature: signature.to_string(),
                };

                let _ = method.save(&mut *tx).await;
                batch_count += 1;
            }

            if signature_hash.len() == 66 {
                let event = DBEvent {
                    signature_hash: signature_hash.to_string(),
                    signature: signature.to_string(),
                };

                let _ = event.save(&mut *tx).await;
                batch_count += 1;
            }

            // Commit transaction every batch_size inserts
            if batch_count >= batch_size {
                tx.commit().await?;
                tx = conn.begin().await?;
                batch_count = 0;
            }

            line_count += 1;
        }

        // Commit any remaining items in the final batch
        if batch_count > 0 {
            tx.commit().await?;
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

// Gas token/USD price oracle
// https://docs.chain.link/data-feeds/price-feeds/addresses
fn get_price_oracles() -> HashMap<u64, String> {
    let mut price_oracles = HashMap::new();

    // Mainnet
    price_oracles.insert(1, "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419".to_string());

    // Base
    price_oracles.insert(
        8453,
        "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70".to_string(),
    );

    // BSC
    price_oracles.insert(56, "0x0567f2323251f0aab15c8dfb1967e4e8a7d42aee".to_string());

    price_oracles.insert(
        42161,
        "0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612".to_string(),
    );

    // Arbitrum
    price_oracles.insert(
        137,
        "0xAB594600376Ec9fD91F8e885dADF0CE036862dE0".to_string(),
    );

    // Metis
    price_oracles.insert(
        1088,
        "0xD4a5Bb03B5D66d9bf81507379302Ac2C2DFDFa6D".to_string(),
    );

    // Optimism
    price_oracles.insert(10, "0x13e3Ee699D1909E989722E753853AE30b17e08c5".to_string());

    // Avalanche
    price_oracles.insert(
        43114,
        "0x0A77230d17318075983913bC2145DB16C7366156".to_string(),
    );

    // Linea
    price_oracles.insert(
        59144,
        "0x3c6Cd9Cc7c7a4c2Cf5a82734CD249D7D593354dA".to_string(),
    );

    // Scroll
    price_oracles.insert(
        534352,
        "0x6bF14CB0A831078629D993FDeBcB182b21A8774C".to_string(),
    );

    // Fantom Opera
    price_oracles.insert(
        250,
        "0x11DdD3d147E5b83D01cee7070027092397d63658".to_string(),
    );

    price_oracles
}
