use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag, calc_blob_gasprice, eip2930::AccessList},
    network::{AnyNetwork, AnyRpcBlock},
    primitives::address,
    providers::{Provider, ProviderBuilder},
    rpc::types::{
        AccessList as AlloyAccessList, TransactionRequest,
        trace::parity::{TraceType, TransactionTrace},
    },
};
use eyre::Result;
use foundry_fork_db::{BlockchainDb, SharedBackend, cache::BlockchainDbMeta};
use revm::{
    Context, ExecuteCommitEvm, InspectEvm, MainBuilder, MainContext,
    context::{BlockEnv, TransactTo, TxEnv},
    context_interface::block::BlobExcessGasAndPrice,
    database::CacheDB,
    primitives::{Address, FixedBytes, TxKind, U256},
};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};

use super::shared_init::TraceMode;
use super::utils::block_cache_key;
use crate::GenericProvider;
use crate::db::txs::models::transaction::Transaction;
use crate::misc::coinbase_bribe::{TraceData, find_coinbase_transfer};
use crate::models::{
    evm_chain::EVMChain,
    mev_opcode::MEVOpcode,
    mev_state_diff::{MEVStateDiff, u256_to_option_b256},
};

pub async fn init_revm_db(
    block_number: u64,
    trace_mode: &Option<TraceMode>,
    rpc_url: &str,
    chain: &EVMChain,
) -> Result<Option<CacheDB<SharedBackend>>> {
    match trace_mode {
        Some(TraceMode::Revm) => {}
        _ => return Ok(None),
    };

    let provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(rpc_url.parse()?);

    let block = get_cached_revm_block(&provider, chain, block_number).await?;

    let meta = BlockchainDbMeta::default()
        .set_chain(chain.chain_id.into())
        .with_block(&block.inner);

    let cache_path = revm_cache_path(block_number, chain)?;

    let db = BlockchainDb::new(meta, Some(cache_path));
    let shared = SharedBackend::spawn_backend(
        Arc::new(provider),
        db,
        Some(BlockId::Number(BlockNumberOrTag::Number(block_number))),
    )
    .await;
    let cache_db = CacheDB::new(shared);

    Ok(Some(cache_db))
}

pub fn revm_cache_path(block_number: u64, chain: &EVMChain) -> Result<PathBuf> {
    Ok(home::home_dir().unwrap().join(format!(
        ".mevlog/.revm-cache/{}/{block_number}.json",
        chain.revm_cache_dir_name()
    )))
}

pub struct RevmBlockContext {
    pub number: u64,
    pub timestamp: u64,
    pub coinbase: Address,
    pub difficulty: U256,
    pub gas_limit: U256,
    pub basefee: U256,
    pub excess_blob_gas: Option<u64>,
    pub blob_gasprice: Option<u128>,
}

impl RevmBlockContext {
    pub fn new(block: &AnyRpcBlock) -> Self {
        let header = &block.header;
        Self {
            number: header.number(),
            timestamp: header.timestamp(),
            coinbase: header.beneficiary(),
            difficulty: header.difficulty(),
            gas_limit: U256::from(header.gas_limit()),
            basefee: U256::from(header.base_fee_per_gas().unwrap_or(0)),
            excess_blob_gas: header.excess_blob_gas(),
            blob_gasprice: header.excess_blob_gas().map(calc_blob_gasprice),
        }
    }
}

async fn fetch_tx_request(
    tx_hash: FixedBytes<32>,
    provider: &Arc<GenericProvider>,
) -> Result<TransactionRequest> {
    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction {tx_hash} not found"))?;

    Ok(TransactionRequest::from_recovered_transaction(
        tx.into_recovered(),
    ))
}

// Traces `targets` by sequential Revm replay against parent state: fork at
// block-1, replay in index order up to the last target, committing each tx.
//
// When `txs` is `Some`, each target's coinbase transfer is written to SQLite the
// moment its trace completes (mid-replay), so an interrupt keeps prior progress;
// those targets are then omitted from the returned map. With `None`, nothing is
// persisted and every target's trace is returned for the caller to consume.
pub async fn revm_block_traced_calls(
    block_number: u64,
    targets: &HashSet<FixedBytes<32>>,
    provider: &Arc<GenericProvider>,
    rpc_url: &str,
    chain: &EVMChain,
    txs: Option<&sqlx::SqlitePool>,
) -> Result<(
    RevmBlockContext,
    HashMap<FixedBytes<32>, Vec<TransactionTrace>>,
)> {
    // A hashes-only block fetch gives both the header (for the block context) and
    // the txs in index order, without pulling every tx body.
    let any_provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(rpc_url.parse()?);
    let block = get_cached_revm_block(&any_provider, chain, block_number).await?;
    let block_context = RevmBlockContext::new(&block);

    let ordered: Vec<FixedBytes<32>> = block.transactions.hashes().collect();
    let mut traced: HashMap<FixedBytes<32>, Vec<TransactionTrace>> = HashMap::new();

    // Replay only as far as the last requested tx; trailing txs are irrelevant.
    let Some(last_index) = ordered.iter().rposition(|h| targets.contains(h)) else {
        return Ok((block_context, traced));
    };

    let parent_block = block_number.saturating_sub(1);
    let mut cache_db = init_revm_db(parent_block, &Some(TraceMode::Revm), rpc_url, chain)
        .await?
        .ok_or_else(|| eyre::eyre!("Failed to initialize Revm fork DB"))?;

    let total = targets.len();
    let mut done = 0;
    for tx_hash in ordered.into_iter().take(last_index + 1) {
        let tx_req = fetch_tx_request(tx_hash, provider).await?;

        if targets.contains(&tx_hash) {
            let calls = revm_tx_calls(tx_hash, &tx_req, &block_context, &mut cache_db)?;
            match txs {
                Some(txs) => {
                    let traces: Vec<TraceData> = calls.into_iter().map(Into::into).collect();
                    let value = find_coinbase_transfer(block_context.coinbase, traces);
                    Transaction::update_coinbase_transfer(tx_hash, value, txs).await?;
                    done += 1;
                    tracing::info!(
                        "Committed coinbase_transfer {done}/{total} in block {block_number}"
                    );
                }
                None => {
                    traced.insert(tx_hash, calls);
                }
            }
        }

        // Commit unconditionally so the next tx sees this tx's state changes; a
        // reverted tx still advances the sender's nonce and pays gas.
        revm_commit_tx(tx_hash, &tx_req, &block_context, &mut cache_db)?;
    }

    Ok((block_context, traced))
}

pub(crate) async fn backfill_revm(
    untraced: &[Transaction],
    provider: &Arc<GenericProvider>,
    chain: &EVMChain,
    rpc_url: &str,
    txs: &sqlx::SqlitePool,
) -> Result<()> {
    let mut by_block: HashMap<u64, HashSet<FixedBytes<32>>> = HashMap::new();
    for tx in untraced {
        by_block
            .entry(tx.block_number)
            .or_default()
            .insert(tx.tx_hash);
    }

    let total_txs = untraced.len();
    let total_blocks = by_block.len();
    tracing::info!(
        "Tracing coinbase payments for {total_txs} txs across {total_blocks} blocks (revm)"
    );

    let mut blocks: Vec<_> = by_block.into_iter().collect();
    blocks.sort_by_key(|(n, _)| *n);

    for (i, (block_number, targets)) in blocks.into_iter().enumerate() {
        tracing::info!(
            "Tracing block {block_number} ({}/{total_blocks}) (revm)",
            i + 1
        );
        // Pass the pool so each target commits to SQLite mid-replay, tx by tx.
        if let Err(e) =
            revm_block_traced_calls(block_number, &targets, provider, rpc_url, chain, Some(txs))
                .await
        {
            tracing::warn!("coinbase_transfer revm trace failed for block {block_number}: {e}");
        }
    }

    Ok(())
}

pub fn revm_affected_addresses(
    _tx_hash: FixedBytes<32>,
    tx_req: &TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<HashSet<Address>> {
    let trace_types = HashSet::from_iter([TraceType::StateDiff]);
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req, block_context);
    });
    let mut evm = evm.build_mainnet_with_inspector(TracingInspector::new(
        TracingInspectorConfig::from_parity_config(&trace_types),
    ));

    let tx_env = evm.tx.clone();
    let res = match evm.inspect_tx(tx_env) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_affected_addresses failed. {:?}", e);
            return Ok(HashSet::new());
        }
    };

    Ok(res.state.keys().cloned().collect())
}

pub async fn revm_affected_addresses_for_tx(
    tx_hash: FixedBytes<32>,
    block_number: u64,
    provider: &Arc<GenericProvider>,
    rpc_url: &str,
    chain: &EVMChain,
) -> Result<HashSet<Address>> {
    let any_provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(rpc_url.parse()?);
    let block = get_cached_revm_block(&any_provider, chain, block_number).await?;
    let block_context = RevmBlockContext::new(&block);

    let ordered: Vec<FixedBytes<32>> = block.transactions.hashes().collect();
    let Some(last_index) = ordered.iter().position(|h| h == &tx_hash) else {
        eyre::bail!("Transaction {tx_hash} not found in block {block_number}");
    };

    let parent_block = block_number.saturating_sub(1);
    let mut cache_db = init_revm_db(parent_block, &Some(TraceMode::Revm), rpc_url, chain)
        .await?
        .ok_or_else(|| eyre::eyre!("Failed to initialize Revm fork DB"))?;

    for current_hash in ordered.into_iter().take(last_index + 1) {
        let tx_req = fetch_tx_request(current_hash, provider).await?;

        if current_hash == tx_hash {
            return revm_affected_addresses(tx_hash, &tx_req, &block_context, &mut cache_db);
        }

        revm_commit_tx(current_hash, &tx_req, &block_context, &mut cache_db)?;
    }

    Ok(HashSet::new())
}

pub async fn revm_state_diff_for_tx(
    tx_hash: FixedBytes<32>,
    block_number: u64,
    provider: &Arc<GenericProvider>,
    rpc_url: &str,
    chain: &EVMChain,
) -> Result<MEVStateDiff> {
    let any_provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(rpc_url.parse()?);
    let block = get_cached_revm_block(&any_provider, chain, block_number).await?;
    let block_context = RevmBlockContext::new(&block);

    let ordered: Vec<FixedBytes<32>> = block.transactions.hashes().collect();
    let Some(last_index) = ordered.iter().position(|h| h == &tx_hash) else {
        eyre::bail!("Transaction {tx_hash} not found in block {block_number}");
    };

    let parent_block = block_number.saturating_sub(1);
    let mut cache_db = init_revm_db(parent_block, &Some(TraceMode::Revm), rpc_url, chain)
        .await?
        .ok_or_else(|| eyre::eyre!("Failed to initialize Revm fork DB"))?;

    for current_hash in ordered.into_iter().take(last_index + 1) {
        let tx_req = fetch_tx_request(current_hash, provider).await?;

        if current_hash == tx_hash {
            return revm_tx_state_diff(tx_hash, &tx_req, &block_context, &mut cache_db);
        }

        revm_commit_tx(current_hash, &tx_req, &block_context, &mut cache_db)?;
    }

    Ok(MEVStateDiff::new())
}

pub async fn revm_opcodes_for_tx(
    tx_hash: FixedBytes<32>,
    block_number: u64,
    provider: &Arc<GenericProvider>,
    rpc_url: &str,
    chain: &EVMChain,
) -> Result<Vec<MEVOpcode>> {
    let any_provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(rpc_url.parse()?);
    let block = get_cached_revm_block(&any_provider, chain, block_number).await?;
    let block_context = RevmBlockContext::new(&block);

    let ordered: Vec<FixedBytes<32>> = block.transactions.hashes().collect();
    let Some(last_index) = ordered.iter().position(|h| h == &tx_hash) else {
        eyre::bail!("Transaction {tx_hash} not found in block {block_number}");
    };

    let parent_block = block_number.saturating_sub(1);
    let mut cache_db = init_revm_db(parent_block, &Some(TraceMode::Revm), rpc_url, chain)
        .await?
        .ok_or_else(|| eyre::eyre!("Failed to initialize Revm fork DB"))?;

    for current_hash in ordered.into_iter().take(last_index + 1) {
        let tx_req = fetch_tx_request(current_hash, provider).await?;

        if current_hash == tx_hash {
            return revm_tx_opcodes(tx_hash, &tx_req, &block_context, &mut cache_db);
        }

        revm_commit_tx(current_hash, &tx_req, &block_context, &mut cache_db)?;
    }

    Ok(vec![])
}

pub fn revm_tx_calls(
    tx_hash: FixedBytes<32>,
    tx_req: &TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<Vec<TransactionTrace>> {
    let trace_types = HashSet::from_iter([TraceType::Trace]);
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req, block_context);
    });
    let mut evm = evm.build_mainnet_with_inspector(TracingInspector::new(
        TracingInspectorConfig::from_parity_config(&trace_types),
    ));

    let tx_env = evm.tx.clone();
    let res = match evm.inspect_tx(tx_env) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_tx_calls {tx_hash} failed. {:?}", e);
            return Ok(vec![]);
        }
    };

    let full_trace = evm
        .into_inspector()
        .into_parity_builder()
        .into_trace_results(&res.result, &trace_types);

    let txs = &full_trace.trace;

    Ok(txs.clone())
}

pub fn revm_tx_opcodes(
    tx_hash: FixedBytes<32>,
    tx_req: &TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<Vec<MEVOpcode>> {
    let trace_types = HashSet::from_iter([TraceType::VmTrace]);
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req, block_context);
    });
    let mut evm = evm.build_mainnet_with_inspector(TracingInspector::new(
        TracingInspectorConfig::from_parity_config(&trace_types),
    ));

    let tx_env = evm.tx.clone();
    let res = match evm.inspect_tx(tx_env) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_tx_opcodes {tx_hash} failed. {:?}", e);
            return Ok(vec![]);
        }
    };

    let full_trace = evm
        .into_inspector()
        .into_parity_builder()
        .into_trace_results(&res.result, &trace_types);

    let mut opcodes = Vec::new();

    if let Some(vm_trace) = &full_trace.vm_trace {
        for op in &vm_trace.ops {
            if let Some(op_str) = &op.op {
                let pc = op.pc;
                let cost = op.cost;
                let gas_left = op.ex.as_ref().map(|ex| ex.used).unwrap_or(0);

                opcodes.push(MEVOpcode::new(pc as u64, op_str.clone(), cost, gas_left));
            }
        }
    }

    Ok(opcodes)
}

pub fn revm_tx_state_diff(
    tx_hash: FixedBytes<32>,
    tx_req: &TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<MEVStateDiff> {
    let trace_types = HashSet::from_iter([TraceType::StateDiff]);
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req, block_context);
    });
    let mut evm = evm.build_mainnet_with_inspector(TracingInspector::new(
        TracingInspectorConfig::from_parity_config(&trace_types),
    ));

    let tx_env = evm.tx.clone();
    let res = match evm.inspect_tx(tx_env) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_tx_state_diff {tx_hash} failed. {:?}", e);
            return Ok(MEVStateDiff::new());
        }
    };

    let mut state_diff = MEVStateDiff::new();

    for (address, account) in res.state.iter() {
        for (slot, slot_state) in account.storage.iter() {
            let original = slot_state.original_value;
            let present = slot_state.present_value;

            if original != present {
                state_diff.add_change(
                    *address,
                    (*slot).into(),
                    u256_to_option_b256(original),
                    u256_to_option_b256(present),
                );
            }
        }
    }

    Ok(state_diff)
}

pub fn revm_commit_tx(
    tx_hash: FixedBytes<32>,
    tx_req: &TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<()> {
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx| {
        apply_tx_env(tx, tx_req, block_context);
    });
    let mut evm = evm.build_mainnet();

    let tx_env = evm.tx.clone();
    // Commit regardless of outcome to advance state; `transact_commit` applies
    // nonce/balance changes even on revert. Only a transact error is worth logging.
    if let Err(e) = evm.transact_commit(tx_env) {
        tracing::warn!("revm_commit_tx {tx_hash} failed. {:?}", e);
    }

    Ok(())
}

const OP_STACK_DEPOSIT_ADDRESS: Address = address!("deaddeaddeaddeaddeaddeaddeaddeaddead0001");

fn apply_block_env(block_env: &mut BlockEnv, block_context: &RevmBlockContext) {
    block_env.number = U256::from(block_context.number);
    block_env.timestamp = U256::from(block_context.timestamp);
    block_env.beneficiary = block_context.coinbase;
    block_env.difficulty = block_context.difficulty;
    block_env.gas_limit = block_context.gas_limit.to::<u64>();
    block_env.basefee = block_context.basefee.to::<u64>();

    if let (Some(excess_blob_gas), Some(blob_gasprice)) =
        (block_context.excess_blob_gas, block_context.blob_gasprice)
    {
        block_env.blob_excess_gas_and_price = Some(BlobExcessGasAndPrice {
            excess_blob_gas,
            blob_gasprice,
        });
    }
}

fn apply_tx_env(tx_env: &mut TxEnv, tx_req: &TransactionRequest, block_context: &RevmBlockContext) {
    tx_env.caller = tx_req.from.expect("from must be set");
    tx_env.kind = match tx_req.to {
        Some(to) => match to {
            TxKind::Call(addr) => TransactTo::Call(addr),
            TxKind::Create => TransactTo::Create,
        },
        None => TransactTo::Create,
    };
    tx_env.data = tx_req.input.input.clone().expect("data must be set");
    tx_env.value = tx_req.value.unwrap_or(U256::ZERO);
    tx_env.gas_limit = tx_req.gas.unwrap_or(21000);

    // Gas price determination:
    // - OP Stack deposit txs (from depositor address with zero gas prices): use block basefee
    // - EIP-1559 txs (max_fee_per_gas set): use max_fee_per_gas, fallback to gas_price if zero
    // - Legacy txs (gas_price set): use gas_price
    // - Invalid: panic if neither max_fee_per_gas nor gas_price is provided
    tx_env.gas_price = if tx_req.from == Some(OP_STACK_DEPOSIT_ADDRESS)
        && tx_req.max_fee_per_gas.unwrap_or(0) == 0
        && tx_req.gas_price.unwrap_or(0) == 0
    {
        block_context.basefee.to::<u128>().max(1)
    } else if let Some(max_fee) = tx_req.max_fee_per_gas {
        if max_fee == 0 {
            tx_req.gas_price.unwrap_or(0)
        } else {
            max_fee
        }
    } else if let Some(gas_price) = tx_req.gas_price {
        gas_price
    } else {
        panic!("Transaction must have either gas_price or max_fee_per_gas")
    };

    tx_env.nonce = tx_req.nonce.unwrap_or(0);
    tx_env.gas_priority_fee = tx_req.max_priority_fee_per_gas;
    tx_env.max_fee_per_blob_gas = tx_req.max_fee_per_blob_gas.unwrap_or(0);
    if let Some(AlloyAccessList(ref list)) = tx_req.access_list {
        tx_env.access_list = AccessList::from(list.clone());
    };
    tx_env.chain_id = Some(1_u64);
    if let Some(ref blob_hashes) = tx_req.blob_versioned_hashes {
        tx_env.blob_hashes = blob_hashes.clone();
    }
}

async fn get_cached_revm_block(
    provider: &impl Provider<AnyNetwork>,
    chain: &EVMChain,
    block_number: u64,
) -> Result<AnyRpcBlock> {
    let cache_key = block_cache_key(chain, block_number);
    let cache_dir = block_cache_dir();
    let block_number_tag = BlockNumberOrTag::Number(block_number);

    if let Ok(cached_data) = cacache::read(&cache_dir, &cache_key).await {
        match serde_json::from_slice::<AnyRpcBlock>(&cached_data) {
            Ok(block) => {
                tracing::debug!("Block {} loaded from cache", block_number);
                return Ok(block);
            }
            Err(e) => {
                tracing::warn!("Failed to deserialize cached block {}: {}", block_number, e);
            }
        }
    }

    let block = provider
        .get_block_by_number(block_number_tag)
        .await?
        .ok_or_else(|| eyre::eyre!("Block {} not found", block_number))?;

    match serde_json::to_vec(&block) {
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

    Ok(block)
}

fn block_cache_dir() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog/.revm-blocks-cache")
}
