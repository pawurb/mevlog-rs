use std::{collections::HashMap, path::PathBuf, process::Command};

use eyre::Result;
use sqlx::SqlitePool;

use crate::models::{
    evm_chain::EVMChain,
    mev_block::{BatchedBlockData, TxData},
    mev_log::MEVLog,
    mev_transaction::MEVTransaction,
};

use crate::misc::symbol_utils::ERC20SymbolsLookup;

fn cryo_cache_dir(chain: &EVMChain) -> PathBuf {
    home::home_dir().unwrap().join(format!(
        ".mevlog/.cryo-cache/{}",
        chain.cryo_cache_dir_name()
    ))
}

pub struct CachedRange {
    pub start: u64,
    pub end: u64,
    pub path: PathBuf,
}

fn scan_cached_ranges(chain: &EVMChain, data_type: &str) -> Vec<CachedRange> {
    let cache_dir = cryo_cache_dir(chain);
    let chain_name = chain.cryo_cache_dir_name();

    if !cache_dir.exists() {
        return vec![];
    }

    let prefix = format!("{}__{}", chain_name, data_type);
    let mut ranges = vec![];

    if let Ok(entries) = std::fs::read_dir(&cache_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|f| f.to_str())
                && filename.starts_with(&prefix)
                && filename.ends_with(".parquet")
                && let Some((start, end)) = parse_block_range_from_filename(filename)
            {
                ranges.push(CachedRange { start, end, path });
            }
        }
    }

    ranges.sort_by_key(|r| r.start);
    ranges
}

fn parse_block_range_from_filename(filename: &str) -> Option<(u64, u64)> {
    let parts: Vec<&str> = filename.split("__").collect();
    if parts.len() < 3 {
        return None;
    }

    let range_part = parts[2].trim_end_matches(".parquet");
    let range_parts: Vec<&str> = range_part.split("_to_").collect();
    if range_parts.len() != 2 {
        return None;
    }

    let start = range_parts[0].parse::<u64>().ok()?;
    let end = range_parts[1].parse::<u64>().ok()?;
    Some((start, end))
}

struct CoverageAnalysis {
    missing_ranges: Vec<(u64, u64)>,
}

fn analyze_coverage(
    cached_ranges: &[CachedRange],
    start_block: u64,
    end_block: u64,
) -> CoverageAnalysis {
    let mut covered = vec![false; (end_block - start_block + 1) as usize];

    for range in cached_ranges {
        if range.end < start_block || range.start > end_block {
            continue;
        }

        let cover_start = range.start.max(start_block);
        let cover_end = range.end.min(end_block);

        for block in cover_start..=cover_end {
            let idx = (block - start_block) as usize;
            covered[idx] = true;
        }
    }

    let mut missing_ranges = vec![];
    let mut gap_start: Option<u64> = None;

    for (i, &is_covered) in covered.iter().enumerate() {
        let block = start_block + i as u64;
        if !is_covered {
            if gap_start.is_none() {
                gap_start = Some(block);
            }
        } else if let Some(start) = gap_start {
            missing_ranges.push((start, block - 1));
            gap_start = None;
        }
    }

    if let Some(start) = gap_start {
        missing_ranges.push((start, end_block));
    }

    CoverageAnalysis { missing_ranges }
}

fn collect_files_for_range(
    cached_ranges: &[CachedRange],
    start_block: u64,
    end_block: u64,
) -> Vec<PathBuf> {
    cached_ranges
        .iter()
        .filter(|r| r.end >= start_block && r.start <= end_block)
        .map(|r| r.path.clone())
        .collect()
}

fn run_cryo_batch(
    data_type: &str,
    start_block: u64,
    end_block: u64,
    chain: &EVMChain,
) -> Result<()> {
    let range = format!("{}:{}", start_block, end_block + 1);
    let cmd = Command::new("cryo")
        .args([
            data_type,
            "-b",
            &range,
            "--rpc",
            &chain.rpc_url,
            "--output-dir",
            cryo_cache_dir(chain).display().to_string().as_str(),
        ])
        .output();

    if let Err(e) = cmd {
        eyre::bail!("cryo batch command failed: {}", e);
    }

    let output = cmd.unwrap();
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eyre::bail!("cryo batch command failed: {}", stderr);
    }

    Ok(())
}

pub async fn fetch_blocks_batch(
    start_block: u64,
    end_block: u64,
    chain: &EVMChain,
    sqlite: &SqlitePool,
    symbols_lookup: &ERC20SymbolsLookup,
    show_erc20_transfer_amount: bool,
) -> Result<BatchedBlockData> {
    if which::which("cryo").is_err() {
        eyre::bail!(
            "'cryo' command not found in PATH. Please install it by running 'cargo install cryo_cli' or visit https://github.com/paradigmxyz/cryo"
        );
    }

    let tx_ranges = scan_cached_ranges(chain, "transactions");
    let tx_coverage = analyze_coverage(&tx_ranges, start_block, end_block);

    for (gap_start, gap_end) in &tx_coverage.missing_ranges {
        run_cryo_batch("txs", *gap_start, *gap_end, chain)?;
    }

    let log_ranges = scan_cached_ranges(chain, "logs");
    let log_coverage = analyze_coverage(&log_ranges, start_block, end_block);

    for (gap_start, gap_end) in &log_coverage.missing_ranges {
        run_cryo_batch("logs", *gap_start, *gap_end, chain)?;
    }

    let tx_ranges = scan_cached_ranges(chain, "transactions");
    let tx_files = collect_files_for_range(&tx_ranges, start_block, end_block);

    let log_ranges = scan_cached_ranges(chain, "logs");
    let log_files = collect_files_for_range(&log_ranges, start_block, end_block);

    let txs_by_block = parse_batch_txs_from_files(&tx_files, start_block, end_block).await?;
    let logs_by_block = parse_batch_logs_from_files(
        &log_files,
        start_block,
        end_block,
        sqlite,
        symbols_lookup,
        show_erc20_transfer_amount,
    )
    .await?;

    Ok(BatchedBlockData {
        txs_by_block,
        logs_by_block,
    })
}

async fn parse_batch_txs_from_files(
    files: &[PathBuf],
    start_block: u64,
    end_block: u64,
) -> Result<HashMap<u64, Vec<TxData>>> {
    let mut txs_by_block: HashMap<u64, Vec<TxData>> = HashMap::new();

    for file_path in files {
        let file = std::fs::File::open(file_path)?;
        let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?;
        let reader = builder.build()?;

        for batch_result in reader {
            let batch = batch_result?;

            for row_idx in 0..batch.num_rows() {
                let (tx_data, block_number) =
                    MEVTransaction::tx_data_from_parquet_row(&batch, row_idx).await?;

                if block_number >= start_block && block_number <= end_block {
                    txs_by_block.entry(block_number).or_default().push(tx_data);
                }
            }
        }
    }

    Ok(txs_by_block)
}

async fn parse_batch_logs_from_files(
    files: &[PathBuf],
    start_block: u64,
    end_block: u64,
    sqlite: &SqlitePool,
    symbols_lookup: &ERC20SymbolsLookup,
    show_erc20_transfer_amount: bool,
) -> Result<HashMap<u64, Vec<MEVLog>>> {
    let mut logs_by_block: HashMap<u64, Vec<MEVLog>> = HashMap::new();

    for file_path in files {
        let file = std::fs::File::open(file_path)?;
        let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?;
        let reader = builder.build()?;

        for batch_result in reader {
            let batch = batch_result?;

            for row_idx in 0..batch.num_rows() {
                let (mev_log, block_number) = MEVLog::from_parquet_row(
                    &batch,
                    row_idx,
                    symbols_lookup,
                    sqlite,
                    show_erc20_transfer_amount,
                )
                .await?;

                if block_number >= start_block && block_number <= end_block {
                    logs_by_block.entry(block_number).or_default().push(mev_log);
                }
            }
        }
    }

    Ok(logs_by_block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_files_exact_match() {
        let ranges = vec![CachedRange {
            start: 100,
            end: 200,
            path: PathBuf::from("file1.parquet"),
        }];
        let files = collect_files_for_range(&ranges, 100, 200);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_collect_files_partial_overlap() {
        let ranges = vec![
            CachedRange {
                start: 100,
                end: 150,
                path: PathBuf::from("file1.parquet"),
            },
            CachedRange {
                start: 151,
                end: 200,
                path: PathBuf::from("file2.parquet"),
            },
        ];
        let files = collect_files_for_range(&ranges, 120, 180);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_collect_files_no_overlap() {
        let ranges = vec![CachedRange {
            start: 100,
            end: 150,
            path: PathBuf::from("file1.parquet"),
        }];
        let files = collect_files_for_range(&ranges, 200, 300);
        assert_eq!(files.len(), 0);
    }

    #[test]
    fn test_analyze_coverage_full_coverage() {
        let ranges = vec![CachedRange {
            start: 100,
            end: 200,
            path: PathBuf::from("file1.parquet"),
        }];
        let coverage = analyze_coverage(&ranges, 100, 200);
        assert!(coverage.missing_ranges.is_empty());
    }

    #[test]
    fn test_analyze_coverage_gap_in_middle() {
        let ranges = vec![
            CachedRange {
                start: 100,
                end: 110,
                path: PathBuf::from("file1.parquet"),
            },
            CachedRange {
                start: 120,
                end: 130,
                path: PathBuf::from("file2.parquet"),
            },
        ];
        let coverage = analyze_coverage(&ranges, 100, 130);
        assert_eq!(coverage.missing_ranges, vec![(111, 119)]);
    }

    #[test]
    fn test_analyze_coverage_no_coverage() {
        let ranges = vec![];
        let coverage = analyze_coverage(&ranges, 100, 110);
        assert_eq!(coverage.missing_ranges, vec![(100, 110)]);
    }

    #[test]
    fn test_parse_block_range_from_filename() {
        assert_eq!(
            parse_block_range_from_filename("ethereum__transactions__00000100_to_00000200.parquet"),
            Some((100, 200))
        );
        assert_eq!(parse_block_range_from_filename("invalid_filename"), None);
    }
}
