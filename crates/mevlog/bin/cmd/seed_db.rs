use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};

use arrow::array::{Array, BinaryArray, StringArray};
use clap::Parser;
use eyre::{OptionExt, Result};
use futures_util::StreamExt;
use mevlog::{
    db::{
        shared::truncate_wal,
        sigs::{
            self,
            models::{chain::Chain, event::Event, method::Method},
        },
    },
    misc::rpc_urls::get_all_chains,
};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use tracing::info;

const SOURCIFY_BASE_URL: &str = "https://export.sourcify.dev";
// Signature dictionary: maps each signature to its text plus 4-byte selector
// and 32-byte hash. Has no type information.
const SIGNATURES_PREFIX: &str = "v2/signatures/";
// Per-compilation signatures: maps a 32-byte hash to its type (function /
// event / error). Has no signature text. Joined with the dictionary above on
// the 32-byte hash to learn whether each signature is a method or an event.
const COMPILED_PREFIX: &str = "v2/compiled_contracts_signatures/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SigType {
    Method,
    Event,
    Both,
}

#[derive(Debug, Parser)]
pub struct SeedDBArgs {
    /// Path of the SQLite database file to create
    #[arg(long)]
    pub output_path: PathBuf,
}

impl SeedDBArgs {
    #[allow(dead_code)]
    pub async fn run(&self) -> Result<()> {
        let db_url = self.output_path.to_string_lossy().into_owned();
        println!("Seeding db at {db_url}");

        let seed_signatures = std::env::var("SEED_SIGNATURES").unwrap_or_default() == "true";

        if seed_signatures {
            info!("Rebuilding database from scratch");
            remove_db_at(&self.output_path)?;
        }

        sigs::init_db(Some(db_url.clone())).await?;
        let sqlite = sigs::conn(Some(db_url)).await?;

        info!("Seeding database");

        self.seed_chains(&sqlite).await?;

        if seed_signatures {
            self.seed_signatures(&sqlite).await?;
        } else {
            info!("SEED_SIGNATURES not set to true, skipping signature seeding");
        }

        // Indexes are not created here to keep the CDN-uploaded DB small; they
        // are built on first CLI use via `sigs::actions::check_and_create_indexes`.
        info!("Truncating WAL");
        truncate_wal(&sqlite).await?;

        info!("Finished seeding database");

        Ok(())
    }

    async fn seed_chains(&self, sqlite: &sqlx::SqlitePool) -> Result<()> {
        tracing::info!("Seeding chains from ChainList.org");

        let chains = get_all_chains().await?;
        let price_oracles = get_price_oracles();

        let mut total_processed = 0;
        let mut total_success = 0;

        for chain in chains {
            total_processed += 1;

            if total_processed % 500 == 0 {
                tracing::info!("Processed {} chains", total_processed);
            }

            if Chain::exists(chain.chain_id as i64, sqlite).await? {
                continue;
            }

            let explorer_url = chain.explorers.first().map(|e| e.url.clone());
            let currency_symbol = chain.native_currency.symbol;

            let db_chain = Chain {
                id: chain.chain_id as i64,
                name: chain.name,
                explorer_url,
                currency_symbol,
                chainlink_oracle: price_oracles.get(&chain.chain_id).cloned(),
                uniswap_v2_pool: None,
            };

            match db_chain.save(sqlite).await {
                Ok(_) => total_success += 1,
                Err(e) => {
                    tracing::warn!("Error saving chain {}: {}", chain.chain_id, e);
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

    async fn seed_signatures(&self, sqlite: &sqlx::SqlitePool) -> Result<()> {
        info!("Seeding signatures from Sourcify export");

        // Pass 1: learn the type of every signature hash from the
        // compiled-contracts export.
        let compiled_files = self
            .download_files(COMPILED_PREFIX, "mevlog_sourcify_compiled")
            .await?;
        let type_map = build_type_map(&compiled_files)?;
        info!("Built type map with {} signature hashes", type_map.len());

        // Pass 2: join the signature dictionary (text + hashes) against the
        // type map and insert each signature into the matching table.
        let signature_files = self
            .download_files(SIGNATURES_PREFIX, "mevlog_sourcify_sigs")
            .await?;

        let batch_size = 5000;
        let mut batch_count = 0;
        let mut total_rows = 0u64;
        let mut method_rows = 0u64;
        let mut event_rows = 0u64;
        let mut skipped_rows = 0u64;

        let mut tx = sqlite.begin().await?;

        for path in &signature_files {
            info!("Processing signatures from {}", path.display());

            let file = std::fs::File::open(path)?;
            let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
            let schema = builder.schema().clone();

            let idx_hash_4 = schema.index_of("signature_hash_4")?;
            let idx_hash_32 = schema.index_of("signature_hash_32")?;
            let idx_signature = schema.index_of("signature")?;

            let reader = builder.with_batch_size(8192).build()?;

            for batch in reader {
                let batch = batch?;

                let col_hash_4 = batch
                    .column(idx_hash_4)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .ok_or_eyre("signature_hash_4 column is not BinaryArray")?;
                let col_hash_32 = batch
                    .column(idx_hash_32)
                    .as_any()
                    .downcast_ref::<BinaryArray>()
                    .ok_or_eyre("signature_hash_32 column is not BinaryArray")?;
                let col_signature = batch
                    .column(idx_signature)
                    .as_any()
                    .downcast_ref::<StringArray>()
                    .ok_or_eyre("signature column is not StringArray")?;

                for row in 0..batch.num_rows() {
                    if col_signature.is_null(row)
                        || col_hash_4.is_null(row)
                        || col_hash_32.is_null(row)
                    {
                        continue;
                    }

                    let hash_32 = col_hash_32.value(row);
                    let Some(key) = hash_key(hash_32) else {
                        continue;
                    };

                    // Skip signatures whose type we couldn't determine.
                    let Some(sig_type) = type_map.get(&key).copied() else {
                        skipped_rows += 1;
                        continue;
                    };

                    let signature = col_signature.value(row).to_string();

                    if matches!(sig_type, SigType::Method | SigType::Both) {
                        let method = Method {
                            signature_hash_4: col_hash_4.value(row).to_vec(),
                            signature: signature.clone(),
                        };
                        let _ = method.save(&mut *tx).await;
                        method_rows += 1;
                        batch_count += 1;
                    }

                    if matches!(sig_type, SigType::Event | SigType::Both) {
                        let event = Event {
                            signature_hash_32: hash_32.to_vec(),
                            signature,
                        };
                        let _ = event.save(&mut *tx).await;
                        event_rows += 1;
                        batch_count += 1;
                    }

                    if batch_count >= batch_size {
                        tx.commit().await?;
                        tx = sqlite.begin().await?;
                        batch_count = 0;
                    }

                    total_rows += 1;
                    if total_rows.is_multiple_of(100_000) {
                        info!("Processed {} signature rows", total_rows);
                    }
                }
            }
        }

        // Commit any remaining items in the final batch
        if batch_count > 0 {
            tx.commit().await?;
        }

        info!(
            "Processed {} signatures: {} methods, {} events, {} skipped (no type)",
            total_rows, method_rows, event_rows, skipped_rows
        );

        Ok(())
    }

    async fn download_files(&self, prefix: &str, sub_dir: &str) -> Result<Vec<PathBuf>> {
        let dir = std::env::temp_dir().join(sub_dir);
        std::fs::create_dir_all(&dir)?;

        let listing_url = format!("{SOURCIFY_BASE_URL}/?prefix={prefix}");
        info!("Fetching file listing from: {}", listing_url);
        let body = reqwest::get(&listing_url)
            .await?
            .error_for_status()?
            .text()
            .await?;

        let keys = parse_listing_keys(&body);
        if keys.is_empty() {
            eyre::bail!("No parquet files found in Sourcify listing for {prefix}");
        }
        info!("Found {} parquet files for {}", keys.len(), prefix);

        let total = keys.len();
        let mut paths = Vec::with_capacity(total);
        for (idx, key) in keys.iter().enumerate() {
            let file_name = key.rsplit('/').next().unwrap_or(key);
            let dest = dir.join(file_name);

            // Reuse already-downloaded files between runs.
            if dest.exists() && std::fs::metadata(&dest)?.len() > 0 {
                info!("[{}/{}] Using cached {}", idx + 1, total, dest.display());
                paths.push(dest);
                continue;
            }

            let url = format!("{SOURCIFY_BASE_URL}/{key}");
            info!("[{}/{}] Downloading {}", idx + 1, total, url);

            // Download to a temp file first so an interrupted run never leaves a
            // truncated file that a later run would treat as a valid cache hit.
            let part = dir.join(format!("{file_name}.part"));
            let response = reqwest::get(&url).await?.error_for_status()?;
            let mut file = std::fs::File::create(&part)?;
            let mut stream = response.bytes_stream();

            while let Some(chunk) = stream.next().await {
                file.write_all(&chunk?)?;
            }
            file.flush()?;
            std::fs::rename(&part, &dest)?;

            paths.push(dest);
        }

        Ok(paths)
    }
}

/// Streams the compiled-contracts parquet files and records, per 32-byte
/// signature hash, whether it is used as a function selector, an event topic,
/// or both. Rows with type `error` (and any other unknown type) are ignored.
fn build_type_map(files: &[PathBuf]) -> Result<HashMap<[u8; 32], SigType>> {
    let mut type_map: HashMap<[u8; 32], SigType> = HashMap::new();
    let mut total_rows = 0u64;

    for path in files {
        info!("Reading signature types from {}", path.display());

        let file = std::fs::File::open(path)?;
        let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;
        let schema = builder.schema().clone();

        let idx_hash_32 = schema.index_of("signature_hash_32")?;
        let idx_type = schema.index_of("signature_type")?;

        let reader = builder.with_batch_size(8192).build()?;

        for batch in reader {
            let batch = batch?;

            let col_hash_32 = batch
                .column(idx_hash_32)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_eyre("signature_hash_32 column is not BinaryArray")?;
            let col_type = batch
                .column(idx_type)
                .as_any()
                .downcast_ref::<StringArray>()
                .ok_or_eyre("signature_type column is not StringArray")?;

            for row in 0..batch.num_rows() {
                if col_hash_32.is_null(row) || col_type.is_null(row) {
                    continue;
                }

                let new_type = match col_type.value(row) {
                    "function" => SigType::Method,
                    "event" => SigType::Event,
                    _ => continue,
                };

                let Some(key) = hash_key(col_hash_32.value(row)) else {
                    continue;
                };

                type_map
                    .entry(key)
                    .and_modify(|existing| {
                        if *existing != new_type {
                            *existing = SigType::Both;
                        }
                    })
                    .or_insert(new_type);

                total_rows += 1;
                if total_rows.is_multiple_of(1_000_000) {
                    info!(
                        "Scanned {} type rows ({} distinct hashes)",
                        total_rows,
                        type_map.len()
                    );
                }
            }
        }
    }

    Ok(type_map)
}

fn hash_key(bytes: &[u8]) -> Option<[u8; 32]> {
    bytes.try_into().ok()
}

fn remove_db_at(path: &Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm", "-journal"] {
        let target = if suffix.is_empty() {
            path.to_path_buf()
        } else {
            PathBuf::from(format!("{}{suffix}", path.display()))
        };

        if target.exists() {
            std::fs::remove_file(&target)?;
            info!("Removed {}", target.display());
        }
    }

    Ok(())
}

fn parse_listing_keys(xml: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut rest = xml;

    while let Some(start) = rest.find("<Key>") {
        let after = &rest[start + "<Key>".len()..];
        let Some(end) = after.find("</Key>") else {
            break;
        };
        let key = &after[..end];
        if key.ends_with(".parquet") {
            keys.push(key.to_string());
        }
        rest = &after[end + "</Key>".len()..];
    }

    keys
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
