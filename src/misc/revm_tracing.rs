use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use alloy::{
    consensus::BlockHeader,
    eips::{BlockId, BlockNumberOrTag},
    network::AnyNetwork,
    node_bindings::{Anvil, AnvilInstance},
    providers::{Provider, ProviderBuilder},
    rpc::types::{
        trace::parity::{TraceType, TransactionTrace},
        Block, TransactionRequest,
    },
};
use eyre::Result;
use foundry_fork_db::{cache::BlockchainDbMeta, BlockchainDb, SharedBackend};
use revm::{
    db::CacheDB,
    inspector_handle_register,
    primitives::{
        calc_blob_gasprice, AccessList, Address, BlobExcessGasAndPrice, BlockEnv, Bytes, CfgEnv,
        CfgEnvWithHandlerCfg, EVMError, EnvWithHandlerCfg, ExecutionResult, FixedBytes, HandlerCfg,
        Output, ResultAndState, SpecId, TransactTo, TxEnv, U256,
    },
    Database, Evm, GetInspector,
};
use revm_inspectors::tracing::{TracingInspector, TracingInspectorConfig};
use tracing::debug;

use super::shared_init::{ConnOpts, TraceMode};
use crate::models::evm_chain::EVMChain;

pub struct RevmUtils {
    pub anvil: AnvilInstance,
    pub cache_db: CacheDB<SharedBackend>,
}

pub async fn init_revm_db(
    block_number: u64,
    conn_opts: &ConnOpts,
    chain: &EVMChain,
) -> Result<Option<RevmUtils>> {
    match conn_opts.trace {
        Some(TraceMode::Revm) => {}
        _ => return Ok(None),
    };

    let Some(rpc_url) = &conn_opts.rpc_url else {
        eyre::bail!("--tracing revm works only with HTTP provider");
    };

    let anvil = Anvil::new()
        .fork(rpc_url)
        .fork_block_number(block_number)
        .spawn();
    debug!("Initializing HTTP Revm provider");

    let provider = ProviderBuilder::new()
        .network::<AnyNetwork>()
        .on_http(anvil.endpoint().parse()?);

    let block = provider
        .get_block_by_number(BlockNumberOrTag::Number(block_number))
        .await?
        .expect("block not found");

    let meta = BlockchainDbMeta::default()
        .with_chain_id(chain.chain_id)
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
    let foundry_revm_cache = home::home_dir().unwrap().join(format!(
        ".foundry/cache/rpc/{}",
        chain.revm_cache_dir_name()
    ));

    if Path::new(&foundry_revm_cache).exists() {
        if foundry_revm_cache.is_dir() {
            Ok(foundry_revm_cache.join(format!("{block_number}/storage.json")))
        } else {
            Ok(foundry_revm_cache.join(format!("{block_number}")))
        }
    } else {
        Ok(home::home_dir()
            .unwrap()
            .join(format!(".mevlog/.revm-cache/{block_number}")))
    }
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
            blob_gasprice: block
                .header
                .excess_blob_gas
                .map(|excess_blob_gas| calc_blob_gasprice(excess_blob_gas, true)),
        }
    }
}

pub fn revm_touching_accounts(
    _tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<HashSet<Address>> {
    let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::LATEST));

    let mut tx_env = Default::default();
    apply_tx_env(&mut tx_env, tx_req);

    let mut block_env = BlockEnv::default();
    apply_block_env(&mut block_env, block_context);

    let env = EnvWithHandlerCfg::new_with_cfg_env(cfg.clone(), block_env, tx_env);

    let trace_types = HashSet::from_iter([TraceType::StateDiff]);
    let mut insp = TracingInspector::new(TracingInspectorConfig::from_parity_config(&trace_types));
    let (trace, _) = match inspect(cache_db, env, &mut insp) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_touching_accounts failed. {:?}", e);
            return Ok(HashSet::new());
        }
    };

    Ok(trace.state.keys().cloned().collect())
}

fn _revm_call_tx(
    tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<Bytes> {
    let mut evm = Evm::builder()
        .with_db(cache_db)
        .modify_block_env(|block| {
            apply_block_env(block, block_context);
        })
        .modify_tx_env(|tx_env| {
            apply_tx_env(tx_env, tx_req);
        })
        .build();

    let ref_tx = match evm.transact() {
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
    _tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<Vec<TransactionTrace>> {
    let cfg = CfgEnvWithHandlerCfg::new(CfgEnv::default(), HandlerCfg::new(SpecId::LATEST));

    let mut tx_env = Default::default();
    apply_tx_env(&mut tx_env, tx_req);

    let mut block_env = BlockEnv::default();
    apply_block_env(&mut block_env, block_context);

    let env = EnvWithHandlerCfg::new_with_cfg_env(cfg.clone(), block_env, tx_env);

    let trace_types = HashSet::from_iter([TraceType::Trace]);
    let mut insp = TracingInspector::new(TracingInspectorConfig::from_parity_config(&trace_types));
    let (trace, _) = match inspect(cache_db, env, &mut insp) {
        Ok(res) => res,
        Err(e) => {
            tracing::warn!("revm_tx_calls failed. {:?}", e);
            return Ok(vec![]);
        }
    };

    let full_trace = insp
        .into_parity_builder()
        .into_trace_results(&trace.result, &trace_types);

    let txs = &full_trace.trace.to_vec();

    Ok(txs.to_vec())
}

pub fn revm_commit_tx(
    tx_hash: FixedBytes<32>,
    tx_req: TransactionRequest,
    block_context: &RevmBlockContext,
    cache_db: &mut CacheDB<SharedBackend>,
) -> Result<()> {
    let mut evm = Evm::builder()
        .with_db(cache_db)
        .modify_block_env(|block| {
            apply_block_env(block, block_context);
        })
        .modify_tx_env(|tx| {
            apply_tx_env(tx, tx_req);
        })
        .build();

    let ref_tx = match evm.transact_commit() {
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
    block_env.coinbase = block_context.coinbase;
    block_env.difficulty = block_context.difficulty;
    block_env.gas_limit = block_context.gas_limit;
    block_env.basefee = block_context.basefee;

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
    tx_env.transact_to = match tx_req.to {
        Some(to) => to,
        None => TransactTo::Create,
    };
    tx_env.data = tx_req.input.input.expect("data must be set");
    tx_env.value = tx_req.value.unwrap_or(U256::ZERO);
    tx_env.gas_limit = tx_req.gas.unwrap_or(21000);
    // For EIP-1559 transactions, gas_price should be set to max_fee_per_gas
    tx_env.gas_price = if let Some(max_fee) = tx_req.max_fee_per_gas {
        U256::from(max_fee)
    } else if let Some(gas_price) = tx_req.gas_price {
        U256::from(gas_price)
    } else {
        panic!("Transaction must have either gas_price or max_fee_per_gas");
    };

    tx_env.nonce = tx_req.nonce;
    tx_env.gas_priority_fee = tx_req.max_priority_fee_per_gas.map(U256::from);
    tx_env.max_fee_per_blob_gas = tx_req.max_fee_per_blob_gas.map(U256::from);
    if let Some(AccessList(list)) = tx_req.access_list {
        tx_env.access_list = list;
    };
    tx_env.chain_id = Some(1_u64);
    if let Some(blob_hashes) = tx_req.blob_versioned_hashes {
        tx_env.blob_hashes = blob_hashes;
    }
}

fn inspect<DB, I>(
    db: DB,
    env: EnvWithHandlerCfg,
    inspector: I,
) -> Result<(ResultAndState, EnvWithHandlerCfg), EVMError<DB::Error>>
where
    DB: Database,
    I: GetInspector<DB>,
{
    let mut evm = revm::Evm::builder()
        .with_db(db)
        .with_external_context(inspector)
        .with_env_with_handler_cfg(env)
        .append_handler_register(inspector_handle_register)
        .build();
    let res = evm.transact()?;
    let (_, env) = evm.into_db_and_env_with_handler_cfg();
    Ok((res, env))
}
