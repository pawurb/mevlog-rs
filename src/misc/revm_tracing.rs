use std::{collections::HashSet, path::PathBuf, sync::Arc};

use alloy::{
    consensus::BlockHeader,
    eips::{calc_blob_gasprice, eip2930::AccessList, BlockId, BlockNumberOrTag},
    network::AnyNetwork,
    node_bindings::{Anvil, AnvilInstance},
    primitives::Bytes,
    providers::{Provider, ProviderBuilder},
    rpc::types::{
        trace::parity::{TraceType, TransactionTrace},
        AccessList as AlloyAccessList, Block, TransactionRequest,
    },
};
use eyre::Result;
use foundry_fork_db::{cache::BlockchainDbMeta, BlockchainDb, SharedBackend};
use revm::{
    context::{
        result::{ExecutionResult, Output},
        BlockEnv, TransactTo, TxEnv,
    },
    context_interface::block::BlobExcessGasAndPrice,
    database::CacheDB,
    primitives::{Address, FixedBytes, TxKind, U256},
    Context, ExecuteCommitEvm, ExecuteEvm, InspectEvm, MainBuilder, MainContext,
};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use tracing::debug;

use super::shared_init::TraceMode;
use crate::models::evm_chain::EVMChain;

pub struct RevmUtils {
    pub anvil: AnvilInstance,
    pub cache_db: CacheDB<SharedBackend>,
}

pub async fn init_revm_db(
    block_number: u64,
    trace_mode: &Option<TraceMode>,
    rpc_url: &str,
    chain: &EVMChain,
) -> Result<Option<RevmUtils>> {
    match trace_mode {
        Some(TraceMode::Revm) => {}
        _ => return Ok(None),
    };

    let anvil = Anvil::new()
        .fork(rpc_url)
        .fork_block_number(block_number)
        .spawn();
    debug!("Initializing HTTP Revm provider");

    let provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .connect_http(anvil.endpoint().parse()?);

    let block = provider
        .get_block_by_number(BlockNumberOrTag::Number(block_number))
        .await?
        .expect("block not found");

    let meta = BlockchainDbMeta::default()
        .set_chain(chain.chain_id.into())
        .with_block(&block.inner);

    let cache_path = revm_cache_path(block_number, chain)?;

    let db = BlockchainDb::new(meta, Some(cache_path));
    let shared = SharedBackend::spawn_backend(
        Arc::new(provider.clone()),
        db,
        Some(BlockId::Number(BlockNumberOrTag::Number(block_number))),
    )
    .await;
    let cache_db = CacheDB::new(shared);

    Ok(Some(RevmUtils { anvil, cache_db }))
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
    pub fn new(block: &Block) -> Self {
        Self {
            number: block.header.number(),
            timestamp: block.header.timestamp(),
            coinbase: block.header.beneficiary,
            difficulty: block.header.difficulty,
            gas_limit: U256::from(block.header.gas_limit),
            basefee: U256::from(block.header.base_fee_per_gas.expect("Base fee missing")),
            excess_blob_gas: block.header.excess_blob_gas,
            blob_gasprice: block.header.excess_blob_gas.map(calc_blob_gasprice),
        }
    }
}

pub fn revm_touching_accounts(
    _tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<HashSet<Address>> {
    let trace_types = HashSet::from_iter([TraceType::StateDiff]);
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req);
    });
    let mut evm = evm.build_mainnet_with_inspector(TracingInspector::new(
        TracingInspectorConfig::from_parity_config(&trace_types),
    ));

    let tx_env = evm.tx.clone();
    let res = match evm.inspect_tx(tx_env) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_touching_accounts failed. {:?}", e);
            return Ok(HashSet::new());
        }
    };

    Ok(res.state.keys().cloned().collect())
}

fn _revm_call_tx(
    tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<Bytes> {
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req);
    });
    let mut evm = evm.build_mainnet();

    let tx_env = evm.tx.clone();
    let ref_tx = match evm.transact(tx_env) {
        Ok(tx) => tx,
        Err(e) => {
            eyre::bail!("_revm_call_tx {tx_hash} failed. {:?}", e);
        }
    };
    let result = ref_tx.result;

    let value = match result {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => {
            eyre::bail!("_revm_call_tx {tx_hash} failed: {result:?}");
        }
    };

    Ok(value)
}

pub fn revm_tx_calls(
    tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<Vec<TransactionTrace>> {
    let trace_types = HashSet::from_iter([TraceType::Trace]);
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx_env| {
        apply_tx_env(tx_env, tx_req.clone());
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

pub fn revm_commit_tx(
    tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<()> {
    let mut evm = Context::mainnet().with_db(cache_db);
    evm.modify_block(|block| {
        apply_block_env(block, block_context);
    });
    evm.modify_tx(|tx| {
        apply_tx_env(tx, tx_req);
    });
    let mut evm = evm.build_mainnet();

    let tx_env = evm.tx.clone();
    let ref_tx = match evm.transact_commit(tx_env) {
        Ok(tx) => tx,
        Err(e) => {
            tracing::warn!("revm_commit_tx {tx_hash} failed. {:?}", e);
            return Ok(());
        }
    };

    match ref_tx {
        ExecutionResult::Success {
            output: Output::Call(value),
            ..
        } => value,
        result => {
            tracing::warn!("revm_commit_tx {tx_hash} failed: {result:?}");
            return Ok(());
        }
    };

    Ok(())
}

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

fn apply_tx_env(tx_env: &mut TxEnv, tx_req: TransactionRequest) {
    tx_env.caller = tx_req.from.expect("from must be set");
    tx_env.kind = match tx_req.to {
        Some(to) => match to {
            TxKind::Call(addr) => TransactTo::Call(addr),
            TxKind::Create => TransactTo::Create,
        },
        None => TransactTo::Create,
    };
    tx_env.data = tx_req.input.input.expect("data must be set");
    tx_env.value = tx_req.value.unwrap_or(U256::ZERO);
    tx_env.gas_limit = tx_req.gas.unwrap_or(21000);
    // For EIP-1559 transactions, gas_price should be set to max_fee_per_gas
    // If max_fee_per_gas is 0, fall back to gas_price
    tx_env.gas_price = if let Some(max_fee) = tx_req.max_fee_per_gas {
        if max_fee == 0 {
            tx_req.gas_price.unwrap_or(0)
        } else {
            max_fee
        }
    } else if let Some(gas_price) = tx_req.gas_price {
        gas_price
    } else {
        panic!("Transaction must have either gas_price or max_fee_per_gas");
    };

    tx_env.nonce = tx_req.nonce.unwrap_or(0);
    tx_env.gas_priority_fee = tx_req.max_priority_fee_per_gas;
    tx_env.max_fee_per_blob_gas = tx_req.max_fee_per_blob_gas.unwrap_or(0);
    if let Some(AlloyAccessList(list)) = tx_req.access_list {
        tx_env.access_list = AccessList::from(list);
    };
    tx_env.chain_id = Some(1_u64);
    if let Some(blob_hashes) = tx_req.blob_versioned_hashes {
        tx_env.blob_hashes = blob_hashes;
    }
}
