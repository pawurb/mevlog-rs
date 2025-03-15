use alloy::network::TransactionResponse;
use alloy::rpc::types::{Filter, TransactionRequest};
use alloy::sol;
use alloy::{eips::BlockNumberOrTag, providers::Provider, rpc::types::Block as AlloyBlock};
use colored::Colorize;
use eyre::Result;
use foundry_fork_db::SharedBackend;
use indicatif::{ProgressBar, ProgressStyle};
use revm::db::CacheDB;
use revm::primitives::{address, Address, FixedBytes};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::{collections::HashMap, fmt};
use tokio::sync::Semaphore;
use tracing::{debug, error};

use crate::misc::coinbase_bribe::{find_coinbase_transfer, TraceData};
use crate::misc::db_actions::PROGRESS_CHARS;
use crate::misc::ens_utils::ENSLookup;
use crate::misc::revm_tracing::{
    init_revm_db, revm_commit_tx, revm_touching_accounts, revm_tx_calls, RevmBlockContext,
};
use crate::misc::rpc_tracing::{rpc_touching_accounts, rpc_tx_calls};
use crate::misc::shared_init::{ConnOpts, TraceMode};
use crate::misc::utils::{ToU64, SEPARATORER};
use crate::models::txs_filter::TxsFilter;
use crate::GenericProvider;

use super::mev_log::MEVLog;
use super::mev_transaction::{MEVTransaction, ReceiptData};
use super::txs_filter::{FromFilter, PositionRange};

sol! {
    #[sol(rpc)]
    contract IPriceOracle {
    function latestRoundData()
        returns (
        uint80 roundId,
        int256 answer,
        uint256 startedAt,
        uint256 updatedAt,
        uint80 answeredInRound
        );
    }
}

const ETH_PRICE_ORACLE: Address = address!("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419");

pub struct TxData {
    req: TransactionRequest,
    tx_hash: FixedBytes<32>,
}

pub struct MEVBlock {
    eth_price: f64,
    block_number: u64,
    mev_transactions: HashMap<u64, MEVTransaction>,
    all_transactions: HashMap<u64, TxData>,
    inner: AlloyBlock,
    revm_context: RevmBlockContext,
    txs_count: u64,
    reversed_order: bool,
}

pub async fn process_block(
    provider: &Arc<GenericProvider>,
    conn: Arc<SqlitePool>,
    block_number: u64,
    ens_lookup: &ENSLookup,
    txs_filter: &TxsFilter,
    conn_opts: &ConnOpts,
) -> Result<()> {
    let revm_utils = init_revm_db(block_number - 1, conn_opts).await?;

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
    )
    .await?;

    mev_block
        .populate_txs(
            txs_filter,
            &conn,
            ens_lookup,
            provider,
            revm_db.as_mut(),
            conn_opts,
        )
        .await?;

    print!("{}", mev_block);

    Ok(())
}

impl MEVBlock {
    pub async fn new(
        block_number: u64,
        position_range: Option<&PositionRange>,
        reversed_order: bool,
        provider: &Arc<GenericProvider>,
        trace_mode: Option<&TraceMode>,
    ) -> Result<Self> {
        let block_number_tag = BlockNumberOrTag::Number(block_number);

        let price_oracle = IPriceOracle::new(ETH_PRICE_ORACLE, provider.clone());
        let eth_price = price_oracle.latestRoundData().call().await?.answer;
        let eth_price = eth_price.low_i64() as f64 / 10e7;

        let block = provider
            .get_block_by_number(block_number_tag)
            .full()
            .await?;
        let Some(block) = block else {
            eyre::bail!("Full block {} not found", block_number);
        };
        let revm_context = RevmBlockContext::new(&block);

        let all_txs: HashMap<u64, TxData> = match trace_mode {
            Some(TraceMode::Revm) => {
                let range = match position_range {
                    Some(range) => range,
                    None => {
                        eyre::bail!("--trace revm mode requires --position argument");
                    }
                };

                block
                    .clone()
                    .into_transactions_vec()
                    .into_iter()
                    .filter_map(|tx| {
                        let tx_index = tx.transaction_index?;
                        if tx_index <= range.to {
                            let tx_hash = tx.tx_hash();
                            Some((
                                tx_index,
                                TxData {
                                    req: tx.into_request(),
                                    tx_hash,
                                },
                            ))
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            _ => HashMap::new(),
        };

        Ok(Self {
            eth_price,
            block_number,
            mev_transactions: HashMap::new(),
            txs_count: block.transactions.len() as u64,
            inner: block,
            revm_context,
            reversed_order,
            all_transactions: all_txs,
        })
    }

    pub async fn populate_txs(
        &mut self,
        filter: &TxsFilter,
        sqlite: &Arc<SqlitePool>,
        ens_lookup: &ENSLookup,
        provider: &Arc<GenericProvider>,
        revm_db: Option<&mut CacheDB<SharedBackend>>,
        conn_opts: &ConnOpts,
    ) -> Result<()> {
        let txs = self.inner.clone().into_transactions_vec();
        for tx in txs {
            let tx_hash = tx.tx_hash();
            let Some(tx_index) = tx.transaction_index else {
                panic!("Tx index not found");
            };

            if let Some(indexes) = &filter.tx_indexes {
                if !indexes.contains(&tx_index) {
                    continue;
                }
            }

            if let Some(position_range) = &filter.tx_position {
                if tx_index < position_range.from || tx_index > position_range.to {
                    continue;
                }
            }

            let tx = tx.clone().into_request();

            let mev_tx = match MEVTransaction::new(
                self.eth_price,
                tx,
                tx_hash,
                tx_index,
                sqlite,
                ens_lookup,
                provider,
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
                Some(FromFilter::Address(from_addr)) => {
                    if &mev_tx.from() != from_addr {
                        continue;
                    }
                }
                Some(FromFilter::ENSName(ens_name)) => {
                    if mev_tx.ens_name() != Some(ens_name) {
                        continue;
                    }
                }
                None => {}
            }

            self.mev_transactions.insert(tx_index, mev_tx);
        }

        self.ingest_logs(filter, sqlite, provider).await?;
        self.non_trace_filter_txs(filter).await?;

        if filter.prefetch_receipts() {
            self.retch_receipts(provider.clone()).await?;
        }

        match conn_opts.trace {
            Some(TraceMode::RPC) => self.trace_txs_rpc(filter, provider).await?,
            Some(TraceMode::Revm) => {
                self.trace_txs_revm(filter, revm_db.expect("Revm must be present"))
                    .await?
            }
            _ => {}
        };

        if !filter.prefetch_receipts() {
            self.retch_receipts(provider.clone()).await?;
        }

        Ok(())
    }

    async fn retch_receipts(&mut self, provider: Arc<GenericProvider>) -> Result<()> {
        let mut handles = vec![];
        let semaphore = Arc::new(Semaphore::new(15));
        let provider = provider.clone();

        for tx_data in self.mev_transactions.iter() {
            let tx_index = *tx_data.0;
            let tx_hash = tx_data.1.tx_hash;
            let permit = semaphore.clone().acquire_owned().await?;
            let provider = provider.clone();
            let handle = tokio::spawn(async move {
                let _permit = permit;

                match provider.get_transaction_receipt(tx_hash).await {
                    Ok(Some(receipt)) => (
                        tx_index,
                        Some(ReceiptData {
                            success: receipt.status(),
                            gas_used: receipt.gas_used,
                            effective_gas_price: receipt.effective_gas_price,
                        }),
                    ),
                    _ => (tx_index, None),
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            let receipt = handle.await?;

            if let (tx_index, Some(receipt)) = receipt {
                let tx = self
                    .mev_transactions
                    .get_mut(&tx_index)
                    .expect("Tx not found");
                tx.receipt_data = Some(receipt);
            }
        }
        Ok(())
    }

    async fn trace_txs_rpc(
        &mut self,
        filter: &TxsFilter,
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

    fn revm_data_cached(&self) -> bool {
        let cache_path = home::home_dir().unwrap().join(format!(
            ".mevlog/.revm-cache/{}.json",
            self.block_number - 1
        ));
        cache_path.exists()
    }

    async fn trace_txs_revm(
        &mut self,
        filter: &TxsFilter,
        revm_db: &mut CacheDB<SharedBackend>,
    ) -> Result<()> {
        let total_txs = self.all_transactions.len() - 1;

        let progress_bar = if !self.revm_data_cached() {
            let pb = ProgressBar::new(total_txs as u64);
            pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} ({eta})")
        .unwrap()
        .progress_chars(PROGRESS_CHARS));

            pb.set_message(format!("Revm: executing transactions 0-{},", total_txs).to_string());
            Some(pb)
        } else {
            None
        };

        for i in 0..=total_txs {
            let i = i as u64;
            let tx_data = self.all_transactions.get(&i).expect("Tx not found");

            if let Some(pb) = &progress_bar {
                pb.set_position(i);
            }

            let Some(mev_tx) = self.mev_transactions.get_mut(&i) else {
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
                    self.mev_transactions.remove(&i);

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

            let coinbase_transfer = find_coinbase_transfer(
                self.revm_context.coinbase,
                calls.into_iter().map(TraceData::from).collect(),
            );

            mev_tx.coinbase_transfer = Some(coinbase_transfer);

            if filter.should_exclude(mev_tx) {
                self.mev_transactions.remove(&i);
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
        sqlite: &Arc<SqlitePool>,
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

            if let Some(indexes) = &filter.tx_indexes {
                if !indexes.contains(&tx_index) {
                    continue;
                }
            }

            let mev_log = match MEVLog::new(first_topic, log.clone(), sqlite).await {
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
                    .any(|log| event_query.matches(&log.signature, &log.source()))
            })
        });

        self.mev_transactions.retain(|_, tx| {
            !filter.not_events.iter().any(|not_event_query| {
                tx.logs()
                    .iter()
                    .any(|log| not_event_query.matches(&log.signature, &log.source()))
            })
        });

        self.mev_transactions.retain(|_, tx| {
            if let Some(method_query) = &filter.match_method {
                method_query.matches(&tx.method_name)
            } else {
                true
            }
        });

        Ok(())
    }
}

impl fmt::Display for MEVBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if std::env::var("QUIET").unwrap_or_default() == "1" {
            return Ok(());
        }

        let mut indexes = self.mev_transactions.keys().collect::<Vec<_>>();
        indexes.sort();
        if !self.reversed_order {
            indexes.reverse();
        }

        for (i, &index) in indexes.iter().enumerate() {
            let tx = &self.mev_transactions[index];
            if i < indexes.len() - 1 {
                writeln!(f, "{}", tx)?;
            } else {
                write!(f, "{}", tx)?;
            }
        }

        if indexes.is_empty() {
            writeln!(f, "{}", "No matching transactions.".yellow())?;
        }

        let timestamp = self.revm_context.timestamp;
        let age = chrono::Utc::now().timestamp() - timestamp as i64;
        let base_fee_gwei = self.revm_context.basefee.to_u64() as f64 / 1000000000.0;
        writeln!(
            f,
            "{}",
            format!(
                "Block {} | Age {} | Base {:.2} gwei | Txs {}/{}",
                self.block_number,
                format_age(age),
                base_fee_gwei,
                indexes.len(),
                self.txs_count,
            )
            .blue()
            .bold()
        )?;
        writeln!(f, "{}", SEPARATORER)?;

        Ok(())
    }
}

fn format_age(seconds: i64) -> String {
    if seconds < 60 {
        format!("{}s", seconds)
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else if seconds < 86400 {
        format!("{}h", seconds / 3600)
    } else {
        format!("{}d", seconds / 86400)
    }
}
