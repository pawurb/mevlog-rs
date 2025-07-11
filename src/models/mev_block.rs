use std::{collections::HashMap, fmt, path::PathBuf, process::Command, sync::Arc};

use alloy::{
    eips::BlockNumberOrTag,
    providers::Provider,
    rpc::types::{trace::parity::Action, Filter, TransactionRequest},
};
use colored::Colorize;
use eyre::Result;
use foundry_fork_db::SharedBackend;
use indicatif::{ProgressBar, ProgressStyle};
use revm::{db::CacheDB, primitives::FixedBytes};
use sqlx::SqlitePool;
use tracing::{debug, error};

use super::{
    mev_log::MEVLog,
    mev_transaction::{MEVTransaction, ReceiptData},
    txs_filter::{AddressFilter, TxsFilter},
};
use crate::{
    misc::{
        args_parsing::PositionRange,
        coinbase_bribe::{find_coinbase_transfer, TraceData},
        db_actions::PROGRESS_CHARS,
        ens_utils::ENSLookup,
        revm_tracing::{
            init_revm_db, revm_cache_path, revm_commit_tx, revm_touching_accounts, revm_tx_calls,
            RevmBlockContext,
        },
        rpc_tracing::{rpc_touching_accounts, rpc_tx_calls},
        shared_init::{ConnOpts, TraceMode},
        symbol_utils::SymbolLookupWorker,
        utils::{ToU64, ETH_TRANSFER, SEPARATORER, UNKNOWN},
    },
    models::{
        evm_chain::EVMChain,
        mev_transaction::{extract_signature, CallExtract},
    },
    GenericProvider,
};

#[derive(Clone, Debug)]
pub struct TxData {
    pub req: TransactionRequest,
    pub tx_hash: FixedBytes<32>,
    pub receipt: ReceiptData,
}

pub struct MEVBlock {
    native_token_price: Option<f64>,
    block_number: u64,
    mev_transactions: HashMap<u64, MEVTransaction>,
    revm_transactions: HashMap<u64, TxData>,
    txs_data: Vec<TxData>,
    revm_context: RevmBlockContext,
    txs_count: u64,
    reversed_order: bool,
    top_metadata: bool,
    chain: EVMChain,
}

#[allow(clippy::too_many_arguments)]
pub async fn process_block(
    provider: &Arc<GenericProvider>,
    conn: &SqlitePool,
    block_number: u64,
    ens_lookup: &ENSLookup,
    symbols_lookup: &SymbolLookupWorker,
    txs_filter: &TxsFilter,
    conn_opts: &ConnOpts,
    chain: &EVMChain,
    native_token_price: Option<f64>,
) -> Result<()> {
    let revm_utils = init_revm_db(block_number - 1, conn_opts, chain).await?;

    let (mut revm_db, _anvil) = match conn_opts.trace {
        Some(TraceMode::Revm) => {
            let utils = revm_utils.expect("Revm must be present");
            (Some(utils.cache_db), Some(utils.anvil))
        }
        _ => (None, None),
    };

    let mut mev_block = MEVBlock::new(
        block_number,
        txs_filter.tx_position.as_ref(),
        txs_filter.reversed_order,
        provider,
        conn_opts.trace.as_ref(),
        txs_filter.top_metadata,
        chain,
        native_token_price,
    )
    .await?;

    mev_block
        .populate_txs(
            txs_filter,
            conn,
            ens_lookup,
            symbols_lookup,
            provider,
            revm_db.as_mut(),
            conn_opts,
        )
        .await?;

    mev_block.print();

    Ok(())
}

impl MEVBlock {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        block_number: u64,
        position_range: Option<&PositionRange>,
        reversed_order: bool,
        provider: &Arc<GenericProvider>,
        trace_mode: Option<&TraceMode>,
        block_info_top: bool,
        chain: &EVMChain,
        native_token_price: Option<f64>,
    ) -> Result<Self> {
        let block_number_tag = BlockNumberOrTag::Number(block_number);

        let block_number_int = block_number_tag.as_number().unwrap();

        let _cmd = Command::new("cryo")
            .args([
                "txs",
                "-b",
                &block_number_int.to_string(),
                "--rpc",
                &chain.rpc_url,
                "--n-chunks",
                "1",
                "--csv",
                "--output-dir",
                cryo_cache_dir().display().to_string().as_str(),
            ])
            .output();

        let file_path = format!(
            "{}/{}__transactions__{block_number_int}_to_{block_number_int}.csv",
            cryo_cache_dir().display(),
            chain.cryo_cache_dir_name()
        );

        if which::which("cryo").is_err() {
            eyre::bail!("'cryo' command not found in PATH. Please install it by running 'cargo install cryo_cli' or visit https://github.com/paradigmxyz/cryo");
        };

        let file = match std::fs::File::open(file_path.clone()) {
            Ok(file) => file,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    eyre::bail!("Error opening CSV file: {e}");
                }

                let backup_file_path = format!(
                    "{}/{}__transactions__0{block_number_int}_to_0{block_number_int}.csv",
                    cryo_cache_dir().display(),
                    chain.cryo_cache_dir_name()
                );

                match std::fs::File::open(backup_file_path) {
                    Ok(file) => file,
                    Err(e) => {
                        if e.kind() == std::io::ErrorKind::NotFound {
                            eyre::bail!("CSV file {file_path} not found. Make sure that 'cryo' command is working and that you have a valid RPC connection.");
                        } else {
                            eyre::bail!("Error opening CSV file: {e}");
                        }
                    }
                }
            }
        };
        let reader = std::io::BufReader::new(file);
        let mut csv_reader = csv::Reader::from_reader(reader);

        let mut txs_data = vec![];

        for result in csv_reader.records() {
            let record = result?;
            let tx_req = match MEVTransaction::req_from_csv(record).await {
                Ok(tx) => tx,
                Err(e) => {
                    eyre::bail!("Error parsing tx req from csv: {}", e);
                }
            };
            txs_data.push(tx_req);
        }

        let block = provider.get_block_by_number(block_number_tag).await?;

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
                    .clone()
                    .into_iter()
                    .enumerate()
                    .filter_map(|(tx_index, tx_data)| {
                        if tx_index <= range.to as usize {
                            Some((tx_index as u64, tx_data))
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
        symbols_lookup: &SymbolLookupWorker,
        provider: &Arc<GenericProvider>,
        revm_db: Option<&mut CacheDB<SharedBackend>>,
        conn_opts: &ConnOpts,
    ) -> Result<()> {
        for (tx_index, tx) in self.txs_data.iter().enumerate() {
            let tx_index = tx_index as u64;
            let tx_hash = tx.tx_hash;

            if let Some(indexes) = &filter.tx_indexes {
                if !indexes.contains(&(tx_index)) {
                    continue;
                }
            }

            if let Some(position_range) = &filter.tx_position {
                if tx_index < position_range.from || tx_index > position_range.to {
                    continue;
                }
            }

            let mev_tx = match MEVTransaction::new(
                self.native_token_price,
                self.chain.clone(),
                tx.req.clone(),
                tx.receipt.clone(),
                tx_hash,
                tx_index,
                sqlite,
                ens_lookup,
                provider,
                filter.top_metadata,
                filter.show_calls,
            )
            .await
            {
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
                    if mev_tx.ens_name() != Some(ens_name) {
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
                    if mev_tx.ens_name() != Some(ens_name) {
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

            if let Some(value_filter) = &filter.value {
                if !value_filter.matches(mev_tx.value()) {
                    continue;
                }
            }

            self.mev_transactions.insert(tx_index, mev_tx);
        }

        self.ingest_logs(filter, sqlite, symbols_lookup, provider)
            .await?;

        // first exclude txs based non-tracing filters
        self.non_trace_filter_txs(filter).await?;

        match conn_opts.trace {
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

        let mut to_remove = vec![];

        for tx_index in tx_indices {
            let mev_tx = self
                .mev_transactions
                .get_mut(&tx_index)
                .expect("Tx not found");
            let tx_hash = mev_tx.tx_hash;
            let touching = rpc_touching_accounts(tx_hash, provider).await?;

            if let Some(touched) = &filter.touching {
                if !touching.contains(touched) {
                    to_remove.push(tx_index);
                    continue;
                }
            }

            let calls = rpc_tx_calls(mev_tx.tx_hash, provider).await?;

            let mut call_extracts = Vec::new();
            for call in calls.clone() {
                if let Some(to) = call.to {
                    let (signature_hash, signature) =
                        extract_signature(&self.chain, Some(&call.input), tx_index, sqlite).await?;
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

            if filter.should_exclude(mev_tx) {
                to_remove.push(tx_index);
            }
        }

        for index in to_remove {
            self.mev_transactions.remove(&index);
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
            let tx_index = tx_index as u64;
            let tx_data = match self.revm_transactions.get(&tx_index) {
                Some(tx_data) => tx_data,
                None => continue,
            };

            if let Some(pb) = &progress_bar {
                pb.set_position(tx_index);
            }

            let Some(mev_tx) = self.mev_transactions.get_mut(&tx_index) else {
                revm_commit_tx(
                    tx_data.tx_hash,
                    tx_data.req.clone(),
                    &self.revm_context,
                    revm_db,
                )?;
                continue;
            };

            if let Some(touched) = &filter.touching {
                let touching = revm_touching_accounts(
                    mev_tx.tx_hash,
                    mev_tx.inner.clone(),
                    &self.revm_context,
                    revm_db,
                )?;

                if !touching.contains(touched) {
                    self.mev_transactions.remove(&tx_index);

                    revm_commit_tx(
                        tx_data.tx_hash,
                        tx_data.req.clone(),
                        &self.revm_context,
                        revm_db,
                    )?;
                    continue;
                }
            }

            let calls = revm_tx_calls(
                tx_data.tx_hash,
                tx_data.req.clone(),
                &self.revm_context,
                revm_db,
            )?;

            let mut call_extracts = Vec::new();
            for call in calls.clone() {
                if let Action::Call(call_action) = call.action {
                    let (signature_hash, signature) =
                        extract_signature(&self.chain, Some(&call_action.input), tx_index, sqlite)
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

            if filter.should_exclude(mev_tx) {
                self.mev_transactions.remove(&tx_index);
            }

            revm_commit_tx(
                tx_data.tx_hash,
                tx_data.req.clone(),
                &self.revm_context,
                revm_db,
            )?;
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
        symbols_lookup: &SymbolLookupWorker,
        provider: &Arc<GenericProvider>,
    ) -> Result<()> {
        let block_number = BlockNumberOrTag::Number(self.block_number);
        let log_filter = Filter::new()
            .from_block(block_number)
            .to_block(block_number);
        let logs = provider.get_logs(&log_filter).await?;

        for log in logs {
            let topics = log.topics();
            let Some(first_topic) = topics.first() else {
                continue;
            };

            let Some(_tx_hash) = log.transaction_hash else {
                debug!("Log without transaction_hash");
                continue;
            };

            let Some(_) = log.log_index else {
                debug!("Log without log_index");
                continue;
            };

            let Some(tx_index) = log.transaction_index else {
                debug!("Log without transaction_index");
                continue;
            };

            if let Some(position_range) = &filter.tx_position {
                if tx_index < position_range.from || tx_index > position_range.to {
                    continue;
                }
            }

            if let Some(indexes) = &filter.tx_indexes {
                if !indexes.contains(&tx_index) {
                    continue;
                }
            }

            let mev_log = match MEVLog::new(first_topic, &log, symbols_lookup, sqlite).await {
                Ok(log) => log,
                Err(e) => {
                    error!("Error: {}", e);
                    continue;
                }
            };

            if let Some(tx) = self.mev_transactions.get_mut(&tx_index) {
                tx.add_log(mev_log);
            }
        }
        Ok(())
    }

    async fn non_trace_filter_txs(&mut self, filter: &TxsFilter) -> Result<()> {
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

        self.mev_transactions.retain(|_, tx| {
            if let Some(method_query) = &filter.match_method {
                let signature_match = method_query.matches(&tx.signature);

                let signature_hash_match = match &tx.signature_hash {
                    Some(hash) => method_query.matches(hash),
                    None => false,
                };

                signature_match || signature_hash_match
            } else {
                true
            }
        });

        Ok(())
    }

    pub fn print(&self) {
        let mev_block_str = format!("{self}");
        print!("{}", escape_html(&mev_block_str));
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
            writeln!(f, "{SEPARATORER}")?;
        }

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
        format_age(age),
        base_fee_gwei,
        block.mev_transactions.len(),
        block.txs_count,
        block.chain.name,
        block.chain.chain_id,
        age_width = 3,
        base_width = 6,
    )
}

fn format_age(seconds: i64) -> String {
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

fn cryo_cache_dir() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog/.cryo-cache")
}
