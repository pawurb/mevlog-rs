use std::{collections::HashMap, fmt, path::PathBuf, process::Command, sync::Arc};

use alloy::{
    eips::BlockNumberOrTag,
    providers::Provider,
    rpc::types::{Block, TransactionRequest, trace::parity::Action},
};
use cacache;
use colored::Colorize;
use eyre::Result;
use foundry_fork_db::SharedBackend;
use indicatif::{ProgressBar, ProgressStyle};
use revm::{
    database::CacheDB,
    primitives::{FixedBytes, TxKind, U256},
};
use sqlx::SqlitePool;
use tracing::error;

use super::{
    mev_log::MEVLog,
    mev_transaction::{MEVTransaction, ReceiptData},
    txs_filter::{AddressFilter, TxsFilter},
};
use crate::{
    GenericProvider,
    misc::{
        args_parsing::PositionRange,
        coinbase_bribe::{TraceData, find_coinbase_transfer},
        db_actions::PROGRESS_CHARS,
        ens_utils::ENSLookup,
        revm_tracing::{
            RevmBlockContext, init_revm_db, revm_cache_path, revm_commit_tx,
            revm_touching_accounts, revm_tx_calls,
        },
        rpc_tracing::{rpc_touching_accounts, rpc_tx_calls},
        shared_init::{OutputFormat, SharedOpts, TraceMode},
        symbol_utils::ERC20SymbolsLookup,
        utils::{ETH_TRANSFER, SEPARATORER, ToU64, UNKNOWN},
    },
    models::{
        evm_chain::EVMChain,
        json::mev_transaction_json::MEVTransactionJson,
        mev_transaction::{CallExtract, extract_signature},
    },
};

#[derive(Clone, Debug)]
pub struct TxData {
    pub req: TransactionRequest,
    pub tx_hash: FixedBytes<32>,
    pub receipt: ReceiptData,
}

pub struct MEVBlock {
    pub native_token_price: Option<f64>,
    pub block_number: u64,
    pub mev_transactions: HashMap<u64, MEVTransaction>,
    pub revm_transactions: HashMap<u64, TxData>,
    pub txs_data: Vec<TxData>,
    pub revm_context: RevmBlockContext,
    pub txs_count: u64,
    pub reversed_order: bool,
    pub top_metadata: bool,
    pub chain: Arc<EVMChain>,
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_block(
    provider: &Arc<GenericProvider>,
    sqlite: &SqlitePool,
    block_number: u64,
    ens_lookup: &ENSLookup,
    symbols_lookup: &ERC20SymbolsLookup,
    txs_filter: &TxsFilter,
    shared_opts: &SharedOpts,
    chain: &Arc<EVMChain>,
    rpc_url: &str,
    native_token_price: Option<f64>,
) -> Result<MEVBlock> {
    if block_number == 0 {
        eyre::bail!("Invalid block number: 0");
    }

    let mut revm_db = init_revm_db(block_number - 1, &shared_opts.trace, rpc_url, chain).await?;

    let mut mev_block = MEVBlock::new(
        block_number,
        txs_filter.tx_position.as_ref(),
        txs_filter.reversed_order,
        provider,
        shared_opts.trace.as_ref(),
        txs_filter.top_metadata,
        chain,
        native_token_price,
    )
    .await?;

    mev_block
        .populate_txs(
            txs_filter,
            sqlite,
            ens_lookup,
            symbols_lookup,
            provider,
            revm_db.as_mut(),
            shared_opts,
        )
        .await?;

    Ok(mev_block)
}

#[hotpath::measure_all]
#[allow(clippy::too_many_arguments)]
impl MEVBlock {
    pub async fn new(
        block_number: u64,
        position_range: Option<&PositionRange>,
        reversed_order: bool,
        provider: &Arc<GenericProvider>,
        trace_mode: Option<&TraceMode>,
        block_info_top: bool,
        chain: &Arc<EVMChain>,
        native_token_price: Option<f64>,
    ) -> Result<Self> {
        if which::which("cryo").is_err() {
            eyre::bail!(
                "'cryo' command not found in PATH. Please install it by running 'cargo install cryo_cli' or visit https://github.com/paradigmxyz/cryo"
            );
        };

        let txs_data = get_txs_data(block_number, chain).await?;

        let block = get_cached_block(provider, chain, block_number).await?;

        let Some(block) = block else {
            eyre::bail!("Block {} not found", block_number);
        };
        let revm_context = RevmBlockContext::new(&block);

        let txs_count = txs_data.len() as u64;

        let revm_transactions: HashMap<u64, TxData> = match trace_mode {
            Some(TraceMode::Revm) => {
                let range = match position_range {
                    Some(range) => range,
                    None => {
                        eyre::bail!("--trace revm mode requires --position argument");
                    }
                };

                txs_data
                    .iter()
                    .enumerate()
                    .filter_map(|(tx_index, tx_data)| {
                        if tx_index <= range.to as usize {
                            Some((tx_index as u64, tx_data.clone()))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => HashMap::new(),
        };

        Ok(Self {
            native_token_price,
            block_number,
            mev_transactions: HashMap::new(),
            txs_count,
            revm_context,
            txs_data,
            reversed_order,
            revm_transactions,
            top_metadata: block_info_top,
            chain: chain.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn populate_txs(
        &mut self,
        filter: &TxsFilter,
        sqlite: &SqlitePool,
        ens_lookup: &ENSLookup,
        symbols_lookup: &ERC20SymbolsLookup,
        provider: &Arc<GenericProvider>,
        revm_db: Option<&mut CacheDB<SharedBackend>>,
        shared_opts: &SharedOpts,
    ) -> Result<()> {
        for (tx_index, tx) in self.txs_data.iter().enumerate() {
            let tx_index = tx_index as u64;
            let tx_hash = tx.tx_hash;

            if let Some(indexes) = &filter.tx_indexes
                && !indexes.contains(&(tx_index))
            {
                continue;
            }

            if let Some(position_range) = &filter.tx_position
                && (tx_index < position_range.from || tx_index > position_range.to)
            {
                continue;
            }

            let mev_tx = MEVTransaction::new(
                self.native_token_price,
                self.chain.clone(),
                &tx.req,
                self.block_number,
                tx.receipt.clone(),
                tx_hash,
                tx_index,
                sqlite,
                ens_lookup,
                provider,
                filter.top_metadata,
                filter.show_calls,
            );

            let mev_tx = hotpath::future!(mev_tx, log = true);

            let mev_tx = match mev_tx.await {
                Ok(tx) => tx,
                Err(e) => {
                    error!("Error: {}", e);
                    continue;
                }
            };

            match &filter.tx_from {
                Some(AddressFilter::Address(from_addr)) => {
                    if &mev_tx.from() != from_addr {
                        continue;
                    }
                }
                Some(AddressFilter::ENSName(ens_name)) => {
                    if mev_tx.from_ens_name() != Some(ens_name) {
                        continue;
                    }
                }
                Some(AddressFilter::CreateCall) => {
                    eyre::bail!("CREATE query works only for --to filter");
                }
                None => {}
            }

            match &filter.tx_to {
                Some(AddressFilter::Address(to_addr)) => {
                    if mev_tx.to() != Some(*to_addr) {
                        continue;
                    }
                }
                Some(AddressFilter::ENSName(ens_name)) => {
                    if mev_tx.to_ens_name() != Some(ens_name) {
                        continue;
                    }
                }
                Some(AddressFilter::CreateCall) => {
                    if mev_tx.to().is_some() {
                        continue;
                    }
                }
                None => {}
            }

            if let Some(value_filter) = &filter.value
                && !value_filter.matches(mev_tx.value())
            {
                continue;
            }

            self.mev_transactions.insert(tx_index, mev_tx);
        }

        self.ingest_logs(filter, sqlite, symbols_lookup).await?;

        // first exclude txs based non-tracing filters
        self.non_trace_filter_txs(filter).await?;

        match shared_opts.trace {
            Some(TraceMode::RPC) => self.trace_txs_rpc(filter, sqlite, provider).await?,
            Some(TraceMode::Revm) => {
                self.trace_txs_revm(filter, sqlite, revm_db.expect("Revm must be present"))
                    .await?
            }
            _ => {}
        };

        Ok(())
    }

    async fn trace_txs_rpc(
        &mut self,
        filter: &TxsFilter,
        sqlite: &SqlitePool,
        provider: &Arc<GenericProvider>,
    ) -> Result<()> {
        let tx_indices: Vec<u64> = self.mev_transactions.keys().cloned().collect();

        for tx_index in tx_indices {
            let mev_tx = self
                .mev_transactions
                .get_mut(&tx_index)
                .expect("Tx not found");
            let tx_hash = mev_tx.tx_hash;

            if let Some(touched) = &filter.touching {
                let touching = rpc_touching_accounts(tx_hash, provider).await?;

                if !touching.contains(touched) {
                    self.mev_transactions.remove(&tx_index);
                    continue;
                }
            }

            let calls = rpc_tx_calls(mev_tx.tx_hash, provider).await?;

            let mut call_extracts = Vec::new();
            for call in &calls {
                if let Some(to) = call.to {
                    let (signature_hash, signature) = extract_signature(
                        Some(&call.input),
                        tx_index,
                        Some(TxKind::Call(to)),
                        sqlite,
                    )
                    .await?;
                    call_extracts.push(CallExtract {
                        from: call.from,
                        to,
                        signature,
                        signature_hash,
                    });
                }
            }
            mev_tx.calls = Some(call_extracts);

            let coinbase_transfer = find_coinbase_transfer(
                self.revm_context.coinbase,
                calls.into_iter().map(TraceData::from).collect(),
            );

            mev_tx.coinbase_transfer = Some(coinbase_transfer);

            if filter.tracing_should_exclude(mev_tx) {
                self.mev_transactions.remove(&tx_index);
            }
        }

        Ok(())
    }

    fn revm_data_cached(&self) -> Result<bool> {
        Ok(revm_cache_path(self.block_number - 1, &self.chain)?.exists())
    }

    async fn trace_txs_revm(
        &mut self,
        filter: &TxsFilter,
        sqlite: &SqlitePool,
        revm_db: &mut CacheDB<SharedBackend>,
    ) -> Result<()> {
        if self.revm_transactions.is_empty() {
            return Ok(());
        }

        let total_txs = self.revm_transactions.len() - 1;

        let progress_bar = if !self.revm_data_cached()? {
            let pb = ProgressBar::new(total_txs as u64);
            pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars(PROGRESS_CHARS));

            pb.set_message(format!("Revm: executing transactions 0-{total_txs},").to_string());
            Some(pb)
        } else {
            None
        };

        for tx_index in 0..=total_txs {
            let mev_tx_data = self.mev_transactions.get(&(tx_index as u64));
            let Some(mev_tx) = mev_tx_data else {
                continue;
            };

            if !mev_tx.receipt.success {
                continue;
            }

            let tx_index = tx_index as u64;
            let tx_data = match self.revm_transactions.get(&tx_index) {
                Some(tx_data) => tx_data,
                None => continue,
            };

            if let Some(pb) = &progress_bar {
                pb.set_position(tx_index);
            }

            let Some(mev_tx) = self.mev_transactions.get_mut(&tx_index) else {
                revm_commit_tx(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
                continue;
            };

            if let Some(touched) = &filter.touching {
                let touching = revm_touching_accounts(
                    mev_tx.tx_hash,
                    &mev_tx.inner,
                    &self.revm_context,
                    revm_db,
                )?;

                if !touching.contains(touched) {
                    self.mev_transactions.remove(&tx_index);

                    revm_commit_tx(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
                    continue;
                }
            }

            let calls = revm_tx_calls(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;

            let mut call_extracts = Vec::new();
            for call in &calls {
                if let Action::Call(call_action) = &call.action {
                    let (signature_hash, signature) = extract_signature(
                        Some(&call_action.input),
                        tx_index,
                        Some(TxKind::Call(call_action.to)),
                        sqlite,
                    )
                    .await?;

                    call_extracts.push(CallExtract {
                        from: call_action.from,
                        to: call_action.to,
                        signature,
                        signature_hash,
                    });
                }
            }

            mev_tx.calls = Some(call_extracts);

            let coinbase_transfer = find_coinbase_transfer(
                self.revm_context.coinbase,
                calls.into_iter().map(TraceData::from).collect(),
            );

            mev_tx.coinbase_transfer = Some(coinbase_transfer);

            if filter.tracing_should_exclude(mev_tx) {
                self.mev_transactions.remove(&tx_index);
            }

            revm_commit_tx(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
        }

        if let Some(pb) = progress_bar {
            pb.finish_with_message("Revm trace complete");
        }

        Ok(())
    }

    async fn ingest_logs(
        &mut self,
        filter: &TxsFilter,
        sqlite: &SqlitePool,
        symbols_lookup: &ERC20SymbolsLookup,
    ) -> Result<()> {
        let logs_data = match get_logs_data(
            self.block_number,
            &self.chain,
            symbols_lookup,
            sqlite,
            filter.show_erc20_transfer_amount,
        )
        .await
        {
            Ok(logs) => logs,
            Err(_e) => {
                // No logs found or failed to parse, continue without logs processing
                return Ok(());
            }
        };

        for mev_log in logs_data {
            let tx_index = mev_log.tx_index;

            if let Some(position_range) = &filter.tx_position
                && (tx_index < position_range.from || tx_index > position_range.to)
            {
                continue;
            }

            if let Some(indexes) = &filter.tx_indexes
                && !indexes.contains(&tx_index)
            {
                continue;
            }

            if let Some(tx) = self.mev_transactions.get_mut(&tx_index) {
                tx.add_log(mev_log);
            }
        }
        Ok(())
    }

    async fn non_trace_filter_txs(&mut self, filter: &TxsFilter) -> Result<()> {
        if filter.failed {
            self.mev_transactions.retain(|_, tx| !tx.receipt.success);
        }

        if let Some(tx_cost) = &filter.tx_cost {
            self.mev_transactions
                .retain(|_, tx| tx_cost.matches(U256::from(tx.gas_tx_cost())));
        }

        if let Some(effective_gas_price) = &filter.gas_price {
            self.mev_transactions
                .retain(|_, tx| effective_gas_price.matches(tx.effective_gas_price()));
        }

        self.mev_transactions.retain(|_, tx| {
            filter.events.iter().all(|event_query| {
                tx.logs()
                    .iter()
                    .any(|log| event_query.matches(&log.signature.signature, &log.source()))
            })
        });

        self.mev_transactions.retain(|_, tx| {
            !filter.not_events.iter().any(|not_event_query| {
                tx.logs()
                    .iter()
                    .any(|log| not_event_query.matches(&log.signature.signature, &log.source()))
            })
        });

        if let Some(method_query) = &filter.match_method {
            self.mev_transactions.retain(|_, tx| {
                let signature_match = method_query.matches(&tx.signature);

                let signature_hash_match = match &tx.signature_hash {
                    Some(hash) => method_query.matches(hash),
                    None => false,
                };

                signature_match || signature_hash_match
            });
        }

        self.mev_transactions.retain(|_, tx| {
            filter.erc20_transfers.iter().all(|transfer_query| {
                tx.logs().iter().any(|log| {
                    log.is_erc20_transfer()
                        && log
                            .signature
                            .amount
                            .is_some_and(|amount| transfer_query.matches(&log.source(), &amount))
                })
            })
        });

        Ok(())
    }

    pub fn print(&self) {
        let mev_block_str = format!("{self}");
        print!("{}", escape_html(&mev_block_str));
    }

    pub fn print_with_format(&self, format: &OutputFormat) {
        match format {
            OutputFormat::Text => self.print(),
            OutputFormat::Json | OutputFormat::JsonStream => self.print_json(),
            OutputFormat::JsonPretty | OutputFormat::JsonPrettyStream => self.print_json_pretty(),
        }
    }

    pub fn print_json(&self) {
        match serde_json::to_string(&self.transactions_json()) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Error serializing to JSON: {e}"),
        }
    }

    pub fn transactions_json(&self) -> Vec<MEVTransactionJson> {
        let mut indices: Vec<_> = self.mev_transactions.keys().collect();
        indices.sort();

        indices
            .into_iter()
            .map(|&index| MEVTransactionJson::from(&self.mev_transactions[&index]))
            .collect()
    }

    pub fn print_json_pretty(&self) {
        match serde_json::to_string_pretty(&self.transactions_json()) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Error serializing to JSON: {e}"),
        }
    }
}

fn escape_html(input: &str) -> String {
    let escaped = html_escape::encode_text(&input);
    let escaped = escaped.replace("-&gt;", "->");
    let escaped = escaped.replace("&lt;Unknown&gt;", UNKNOWN);
    let escaped = escaped.replace("&lt;ETH transfer&gt;", ETH_TRANSFER);
    escaped.to_string()
}

impl fmt::Display for MEVBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if std::env::var("QUIET").unwrap_or_default() == "1" {
            return Ok(());
        }

        writeln!(f, "{SEPARATORER}")?;

        let mut indexes = self.mev_transactions.keys().collect::<Vec<_>>();
        indexes.sort();

        if indexes.is_empty() {
            writeln!(
                f,
                "{:width$} {}",
                block_metadata(self).blue().bold(),
                "No matching transactions".yellow(),
                width = 53
            )?;
            return Ok(());
        }

        if self.top_metadata {
            writeln!(f, "{SEPARATORER}")?;
            writeln!(f, "{}", block_metadata(self).blue().bold())?;
        }

        if !self.reversed_order {
            indexes.reverse();
        }

        for (i, &index) in indexes.iter().enumerate() {
            let tx = &self.mev_transactions[index];
            if i < indexes.len() - 1 {
                writeln!(f, "{tx}")?;
            } else {
                write!(f, "{tx}")?;
            }
        }

        if !self.top_metadata {
            writeln!(f, "{}", block_metadata(self).blue().bold())?;
        }
        writeln!(f, "{SEPARATORER}")?;

        Ok(())
    }
}

fn block_metadata(block: &MEVBlock) -> String {
    let timestamp = block.revm_context.timestamp;
    let age = chrono::Utc::now().timestamp() - timestamp as i64;
    let base_fee_gwei = block.revm_context.basefee.to_u64() as f64 / 1000000000.0;

    format!(
        "{} | Age {:age_width$} | Base {:base_width$.2} gwei | Txs {}/{} | {} [{}]",
        block.block_number,
        format_block_age(age),
        base_fee_gwei,
        block.mev_transactions.len(),
        block.txs_count,
        block.chain.name,
        block.chain.chain_id,
        age_width = 3,
        base_width = 6,
    )
}

pub fn format_block_age(seconds: i64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    }
}

fn find_matching_parquet_file(
    chain: &EVMChain,
    data_type: &str,
    block_number: u64,
) -> Result<Option<PathBuf>> {
    let cache_dir = cryo_cache_dir(chain);

    // Format block number with leading zeros (8 digits)
    let formatted_block = format!("{block_number:0>8}");

    // Pattern: {chain_prefix}__{data_type}__{formatted_block}_to_{formatted_block}.parquet
    let expected_filename = format!(
        "{}__{}__{}_to_{}.parquet",
        chain.cryo_cache_dir_name(),
        data_type,
        formatted_block,
        formatted_block
    );

    let expected_path = cache_dir.join(&expected_filename);

    if expected_path.exists() {
        return Ok(Some(expected_path));
    }

    Ok(None)
}

fn cryo_cache_dir(chain: &EVMChain) -> PathBuf {
    home::home_dir().unwrap().join(format!(
        ".mevlog/.cryo-cache/{}",
        chain.cryo_cache_dir_name()
    ))
}

async fn get_txs_data(block_number: u64, chain: &EVMChain) -> Result<Vec<TxData>> {
    let txs_data = match try_parse_txs_file(block_number, chain).await {
        Ok(txs_data) => txs_data,
        Err(_e) => {
            let cmd = Command::new("cryo")
                .args([
                    "txs",
                    "-b",
                    &block_number.to_string(),
                    "--rpc",
                    &chain.rpc_url,
                    "--n-chunks",
                    "1",
                    "--output-dir",
                    cryo_cache_dir(chain).display().to_string().as_str(),
                ])
                .output();

            if cmd.is_err() {
                eyre::bail!("cryo command failed: {}", cmd.err().unwrap());
            }

            try_parse_txs_file(block_number, chain).await?
        }
    };

    Ok(txs_data)
}

async fn try_parse_txs_file(block_number: u64, chain: &EVMChain) -> Result<Vec<TxData>> {
    let file_path = match find_matching_parquet_file(chain, "transactions", block_number)? {
        Some(matching_path) => matching_path,
        None => {
            let expected_pattern = format!(
                "{}/{}__transactions__*{block_number}_to_*{block_number}.parquet",
                cryo_cache_dir(chain).display(),
                chain.cryo_cache_dir_name()
            );
            eyre::bail!(
                "No matching transactions Parquet file found (pattern: {expected_pattern}). Make sure that 'cryo' command is working and that you have a valid RPC connection."
            );
        }
    };

    let file = std::fs::File::open(file_path)?;
    let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?;
    let reader = builder.build()?;

    let mut txs_data = vec![];

    for batch_result in reader {
        let batch = batch_result?;

        for row_idx in 0..batch.num_rows() {
            let tx_req = match MEVTransaction::tx_data_from_parquet_row(&batch, row_idx).await {
                Ok(tx) => tx,
                Err(e) => {
                    eyre::bail!("Error parsing tx req from parquet: {}", e);
                }
            };
            txs_data.push(tx_req);
        }
    }

    Ok(txs_data)
}

async fn get_logs_data(
    block_number: u64,
    chain: &EVMChain,
    symbols_lookup: &ERC20SymbolsLookup,
    sqlite: &SqlitePool,
    show_erc20_transfer_amount: bool,
) -> Result<Vec<MEVLog>> {
    let logs_data = match try_parse_logs_file(
        block_number,
        chain,
        symbols_lookup,
        sqlite,
        show_erc20_transfer_amount,
    )
    .await
    {
        Ok(logs_data) => logs_data,
        Err(_e) => {
            let cmd = Command::new("cryo")
                .args([
                    "logs",
                    "-b",
                    &block_number.to_string(),
                    "--rpc",
                    &chain.rpc_url,
                    "--n-chunks",
                    "1",
                    "--output-dir",
                    cryo_cache_dir(chain).display().to_string().as_str(),
                ])
                .output();

            if cmd.is_err() {
                eyre::bail!("cryo logs command failed: {}", cmd.err().unwrap());
            }

            try_parse_logs_file(
                block_number,
                chain,
                symbols_lookup,
                sqlite,
                show_erc20_transfer_amount,
            )
            .await?
        }
    };

    Ok(logs_data)
}

async fn try_parse_logs_file(
    block_number: u64,
    chain: &EVMChain,
    symbols_lookup: &ERC20SymbolsLookup,
    sqlite: &SqlitePool,
    show_erc20_transfer_amount: bool,
) -> Result<Vec<MEVLog>> {
    let file_path = match find_matching_parquet_file(chain, "logs", block_number)? {
        Some(matching_path) => matching_path,
        None => {
            let expected_pattern = format!(
                "{}/{}__logs__*{block_number}_to_*{block_number}.parquet",
                cryo_cache_dir(chain).display(),
                chain.cryo_cache_dir_name()
            );
            eyre::bail!(
                "No matching logs Parquet file found (pattern: {expected_pattern}), continuing without logs processing"
            );
        }
    };

    let file = std::fs::File::open(file_path)?;
    let builder = parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder::try_new(file)?;
    let reader = builder.build()?;

    let mut logs_data = vec![];

    for batch_result in reader {
        let batch = batch_result?;

        for row_idx in 0..batch.num_rows() {
            let mev_log = match MEVLog::from_parquet_row(
                &batch,
                row_idx,
                symbols_lookup,
                sqlite,
                show_erc20_transfer_amount,
            )
            .await
            {
                Ok(log) => log,
                Err(e) => {
                    eyre::bail!("Error parsing log from parquet: {}", e);
                }
            };
            logs_data.push(mev_log);
        }
    }

    Ok(logs_data)
}

pub fn block_cache_key(chain: &EVMChain, block_number: u64) -> String {
    format!("{}-{}", chain.name, block_number)
}

fn block_cache_dir() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog/.blocks-cache")
}

async fn get_cached_block(
    provider: &Arc<GenericProvider>,
    chain: &EVMChain,
    block_number: u64,
) -> Result<Option<Block>> {
    let cache_key = block_cache_key(chain, block_number);
    let cache_dir = block_cache_dir();
    let block_number_tag = BlockNumberOrTag::Number(block_number);

    if let Ok(cached_data) = cacache::read(&cache_dir, &cache_key).await {
        match serde_json::from_slice::<Block>(&cached_data) {
            Ok(block) => {
                tracing::debug!("Block {} loaded from cache", block_number);
                return Ok(Some(block));
            }
            Err(e) => {
                tracing::warn!("Failed to deserialize cached block {}: {}", block_number, e);
            }
        }
    }

    let block = provider.get_block_by_number(block_number_tag).await?;

    if let Some(ref block_data) = block {
        match serde_json::to_vec(block_data) {
            Ok(serialized_block) => {
                if let Err(e) = cacache::write(&cache_dir, &cache_key, &serialized_block).await {
                    tracing::warn!("Failed to cache block {}: {}", block_number, e);
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to serialize block {} for caching: {}",
                    block_number,
                    e
                );
            }
        }
    }

    Ok(block)
}
