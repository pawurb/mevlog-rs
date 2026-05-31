use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use alloy::{
    eips::BlockNumberOrTag,
    providers::Provider,
    rpc::types::{Block, TransactionRequest, trace::parity::Action},
};
use cacache;
use eyre::Result;
use foundry_fork_db::SharedBackend;
use revm::{
    database::CacheDB,
    primitives::{FixedBytes, TxKind},
};
use sqlx::SqlitePool;
use tracing::error;

use super::{
    mev_log::MEVLog,
    mev_transaction::{MEVTransaction, ReceiptData},
};
use crate::{
    GenericProvider,
    db::txs::models::transaction::Transaction,
    misc::{
        coinbase_bribe::{TraceData, find_coinbase_transfer},
        revm_tracing::{
            RevmBlockContext, init_revm_db, revm_commit_tx, revm_tx_calls, revm_tx_opcodes,
            revm_tx_state_diff,
        },
        rpc_tracing::{rpc_tx_calls, rpc_tx_opcodes, rpc_tx_state_diff},
        shared_init::{OutputFormat, SharedOpts, TraceMode},
    },
    models::{
        evm_chain::EVMChain,
        json::mev_transaction_json::{
            JsonSerializeOpts, MEVTransactionJson, serialize_transactions_json,
        },
        mev_transaction::{CallExtract, extract_signature},
    },
};

#[derive(Clone, Debug)]
pub struct TxData {
    pub req: TransactionRequest,
    pub tx_hash: FixedBytes<32>,
    pub receipt: ReceiptData,
}

pub struct PreFetchedBlockData {
    pub txs_data: Vec<TxData>,
    pub logs_data: Vec<MEVLog>,
}

pub struct BatchedBlockData {
    pub txs_by_block: HashMap<u64, Vec<Transaction>>,
    pub logs_by_block: HashMap<u64, Vec<MEVLog>>,
}

pub struct MEVBlock {
    pub native_token_price: Option<f64>,
    pub block_number: u64,
    pub mev_transactions: HashMap<u64, MEVTransaction>,
    pub revm_transactions: HashMap<u64, TxData>,
    pub txs_data: Vec<TxData>,
    pub revm_context: RevmBlockContext,
    pub txs_count: u64,
    pub chain: Arc<EVMChain>,
}

#[allow(clippy::too_many_arguments)]
pub async fn generate_block(
    provider: &Arc<GenericProvider>,
    sqlite: &SqlitePool,
    block_number: u64,
    tx_indexes: Option<&HashSet<u64>>,
    top_metadata: bool,
    shared_opts: &SharedOpts,
    chain: &Arc<EVMChain>,
    rpc_url: &str,
    native_token_price: Option<f64>,
    include_logs: bool,
    pre_fetched: PreFetchedBlockData,
) -> Result<MEVBlock> {
    if block_number == 0 {
        eyre::bail!("Invalid block number: 0");
    }

    let mut revm_db =
        init_revm_db(block_number - 1, &shared_opts.evm_trace, rpc_url, chain).await?;

    // Cap revm replay at the highest requested tx index; without a selection,
    // replay the whole block.
    let max_index = tx_indexes.and_then(|indexes| indexes.iter().max().copied());

    let mut mev_block = MEVBlock::new(
        block_number,
        max_index,
        provider,
        shared_opts.evm_trace.as_ref(),
        chain,
        native_token_price,
        pre_fetched.txs_data,
    )
    .await?;

    mev_block
        .populate_txs(
            tx_indexes,
            top_metadata,
            sqlite,
            provider,
            revm_db.as_mut(),
            shared_opts,
            include_logs,
            pre_fetched.logs_data,
        )
        .await?;

    Ok(mev_block)
}

#[hotpath::measure_all(future = true)]
#[allow(clippy::too_many_arguments)]
impl MEVBlock {
    pub async fn new(
        block_number: u64,
        max_index: Option<u64>,
        provider: &Arc<GenericProvider>,
        trace_mode: Option<&TraceMode>,
        chain: &Arc<EVMChain>,
        native_token_price: Option<f64>,
        txs_data: Vec<TxData>,
    ) -> Result<Self> {
        let block = get_cached_block(provider, chain, block_number).await?;

        let Some(block) = block else {
            eyre::bail!("Block {} not found", block_number);
        };
        let revm_context = RevmBlockContext::new(&block);

        let txs_count = txs_data.len() as u64;

        let revm_transactions: HashMap<u64, TxData> = match trace_mode {
            Some(TraceMode::Revm) => {
                // Without a selection, replay/trace the whole block.
                let max_index = max_index.map(|max| max as usize);

                txs_data
                    .iter()
                    .enumerate()
                    .filter_map(|(tx_index, tx_data)| {
                        if max_index.is_none_or(|max| tx_index <= max) {
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
            revm_transactions,
            chain: chain.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn populate_txs(
        &mut self,
        tx_indexes: Option<&HashSet<u64>>,
        top_metadata: bool,
        sqlite: &SqlitePool,
        provider: &Arc<GenericProvider>,
        revm_db: Option<&mut CacheDB<SharedBackend>>,
        shared_opts: &SharedOpts,
        include_logs: bool,
        logs_data: Vec<MEVLog>,
    ) -> Result<()> {
        for (tx_index, tx) in self.txs_data.iter().enumerate() {
            let tx_index = tx_index as u64;
            let tx_hash = tx.tx_hash;

            if let Some(indexes) = tx_indexes
                && !indexes.contains(&(tx_index))
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
                top_metadata,
                shared_opts.evm_calls,
                include_logs,
                shared_opts.evm_ops,
                shared_opts.evm_state_diff,
            );

            let mev_tx = hotpath::future!(mev_tx, log = true);

            let mev_tx = match mev_tx.await {
                Ok(tx) => tx,
                Err(e) => {
                    error!("Error: {}", e);
                    continue;
                }
            };

            self.mev_transactions.insert(tx_index, mev_tx);
        }

        self.ingest_logs(tx_indexes, logs_data).await?;

        match shared_opts.evm_trace {
            Some(TraceMode::RPC) => self.trace_txs_rpc(shared_opts, sqlite, provider).await?,
            Some(TraceMode::Revm) => {
                self.trace_txs_revm(shared_opts, sqlite, revm_db.expect("Revm must be present"))
                    .await?
            }
            _ => {}
        };

        Ok(())
    }

    async fn trace_txs_rpc(
        &mut self,
        shared_opts: &SharedOpts,
        sqlite: &SqlitePool,
        provider: &Arc<GenericProvider>,
    ) -> Result<()> {
        let tx_indices: Vec<u64> = self.mev_transactions.keys().cloned().collect();

        for tx_index in tx_indices {
            let mev_tx = self
                .mev_transactions
                .get_mut(&tx_index)
                .expect("Tx not found");

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

            if shared_opts.evm_ops {
                let opcodes = rpc_tx_opcodes(mev_tx.tx_hash, provider).await?;
                mev_tx.opcodes = Some(opcodes);
            }

            if shared_opts.evm_state_diff {
                let state_diff = rpc_tx_state_diff(mev_tx.tx_hash, provider).await?;
                mev_tx.state_diff = Some(state_diff);
            }

            let coinbase_transfer = find_coinbase_transfer(
                self.revm_context.coinbase,
                calls.into_iter().map(TraceData::from).collect(),
            );

            mev_tx.coinbase_transfer = Some(coinbase_transfer);
        }

        Ok(())
    }

    async fn trace_txs_revm(
        &mut self,
        shared_opts: &SharedOpts,
        sqlite: &SqlitePool,
        revm_db: &mut CacheDB<SharedBackend>,
    ) -> Result<()> {
        if self.revm_transactions.is_empty() {
            return Ok(());
        }

        let total_txs = self.revm_transactions.len() - 1;

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

            let Some(mev_tx) = self.mev_transactions.get_mut(&tx_index) else {
                revm_commit_tx(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
                continue;
            };

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

            if shared_opts.evm_ops {
                let opcodes =
                    revm_tx_opcodes(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
                mev_tx.opcodes = Some(opcodes);
            }

            if shared_opts.evm_state_diff {
                let state_diff =
                    revm_tx_state_diff(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
                mev_tx.state_diff = Some(state_diff);
            }

            let coinbase_transfer = find_coinbase_transfer(
                self.revm_context.coinbase,
                calls.into_iter().map(TraceData::from).collect(),
            );

            mev_tx.coinbase_transfer = Some(coinbase_transfer);

            revm_commit_tx(tx_data.tx_hash, &tx_data.req, &self.revm_context, revm_db)?;
        }

        Ok(())
    }

    async fn ingest_logs(
        &mut self,
        tx_indexes: Option<&HashSet<u64>>,
        logs_data: Vec<MEVLog>,
    ) -> Result<()> {
        for mev_log in logs_data {
            let tx_index = mev_log.tx_index;

            if let Some(indexes) = tx_indexes
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

    pub fn print_with_format(&self, format: &OutputFormat, json_opts: JsonSerializeOpts) {
        match format {
            OutputFormat::Json => self.print_json(json_opts),
            OutputFormat::JsonPretty => self.print_json_pretty(json_opts),
        }
    }

    pub fn print_json(&self, json_opts: JsonSerializeOpts) {
        match serialize_transactions_json(&self.transactions_json(), json_opts, false) {
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

    pub fn print_json_pretty(&self, json_opts: JsonSerializeOpts) {
        match serialize_transactions_json(&self.transactions_json(), json_opts, true) {
            Ok(json) => println!("{json}"),
            Err(e) => eprintln!("Error serializing to JSON: {e}"),
        }
    }
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
