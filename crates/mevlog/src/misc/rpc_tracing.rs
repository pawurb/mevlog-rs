use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use alloy::{
    primitives::{B256, TxHash},
    providers::ext::DebugApi,
    rpc::types::trace::geth::{
        CallFrame, DiffMode, GethDebugBuiltInTracerType, GethDebugTracerType,
        GethDebugTracingOptions, GethTrace, PreStateConfig, PreStateFrame,
    },
};
use eyre::Result;
use revm::primitives::{Address, FixedBytes};
use tracing::{debug, info, warn};

use crate::{
    GenericProvider,
    db::txs::models::{block::Block, transaction::Transaction},
    misc::coinbase_bribe::{TraceData, find_coinbase_transfer},
    models::state_diff::StateDiff,
};

#[hotpath::measure(log = true, future = true)]
pub(crate) async fn rpc_tx_calls(
    tx_hash: TxHash,
    provider: &Arc<GenericProvider>,
) -> Result<Vec<CallFrame>> {
    let tracing_opts = GethDebugTracingOptions::default();
    let tracing_opts = tracing_opts.with_tracer(GethDebugTracerType::BuiltInTracer(
        GethDebugBuiltInTracerType::CallTracer,
    ));
    let trace = match provider
        .debug_trace_transaction(tx_hash, tracing_opts)
        .await
    {
        Ok(trace) => trace,
        Err(e) => {
            tracing::error!("Error tracing tx: {}", e);
            eyre::bail!("Error tracing tx: {}", e);
        }
    };

    let trace = match trace {
        GethTrace::CallTracer(frame) => frame,
        _ => unreachable!(),
    };
    let mut all_calls = Vec::new();

    collect_calls(&trace, &mut all_calls);
    Ok(all_calls)
}

#[hotpath::measure(log = true, future = true)]
pub(crate) async fn rpc_affected_addresses(
    tx_hash: TxHash,
    provider: &Arc<GenericProvider>,
) -> Result<HashSet<Address>> {
    let tracing_opts = GethDebugTracingOptions::default();
    let tracing_opts = tracing_opts.with_tracer(GethDebugTracerType::BuiltInTracer(
        GethDebugBuiltInTracerType::PreStateTracer,
    ));

    let conf = PreStateConfig {
        diff_mode: Some(true),
        disable_code: Some(true),
        disable_storage: Some(false),
    };

    let tracing_opts = tracing_opts.with_prestate_config(conf);

    let trace = match provider
        .debug_trace_transaction(tx_hash, tracing_opts)
        .await
    {
        Ok(trace) => trace,
        Err(e) => {
            tracing::error!("Error tracing tx: {}", e);
            eyre::bail!("Error tracing tx: {}", e);
        }
    };

    let (pre, post) = match trace {
        GethTrace::PreStateTracer(PreStateFrame::Diff(DiffMode { pre, post })) => (pre, post),
        _ => unreachable!(),
    };

    Ok(pre.keys().chain(post.keys()).copied().collect())
}

fn collect_calls(frame: &CallFrame, result: &mut Vec<CallFrame>) {
    result.push(frame.clone());

    for call in &frame.calls {
        collect_calls(call, result);
    }
}

#[hotpath::measure(log = true, future = true)]
pub(crate) async fn rpc_tx_state_diff(
    tx_hash: TxHash,
    provider: &Arc<GenericProvider>,
) -> Result<StateDiff> {
    let tracing_opts = GethDebugTracingOptions::default();
    let tracing_opts = tracing_opts.with_tracer(GethDebugTracerType::BuiltInTracer(
        GethDebugBuiltInTracerType::PreStateTracer,
    ));

    let conf = PreStateConfig {
        diff_mode: Some(true),
        disable_code: Some(true),
        disable_storage: Some(false),
    };

    let tracing_opts = tracing_opts.with_prestate_config(conf);

    let trace = match provider
        .debug_trace_transaction(tx_hash, tracing_opts)
        .await
    {
        Ok(trace) => trace,
        Err(e) => {
            tracing::error!("Error tracing tx state diff: {}", e);
            return Ok(StateDiff::new());
        }
    };

    let (pre_state, post_state) = match trace {
        GethTrace::PreStateTracer(PreStateFrame::Diff(DiffMode { pre, post })) => (pre, post),
        _ => {
            tracing::warn!("Unexpected trace type for state diff tracing");
            return Ok(StateDiff::new());
        }
    };

    let mut state_diff = StateDiff::new();

    let all_addresses: HashSet<_> = pre_state.keys().chain(post_state.keys()).collect();

    for address in all_addresses {
        let pre_account = pre_state.get(address);
        let post_account = post_state.get(address);

        let pre_storage = pre_account.map(|a| &a.storage);
        let post_storage = post_account.map(|a| &a.storage);

        let all_slots: HashSet<&B256> = pre_storage
            .map(|s| s.keys().collect::<HashSet<_>>())
            .unwrap_or_default()
            .into_iter()
            .chain(
                post_storage
                    .map(|s| s.keys().collect::<HashSet<_>>())
                    .unwrap_or_default(),
            )
            .collect();

        for slot in all_slots {
            let pre_value = pre_storage.and_then(|s| s.get(slot)).copied();
            let post_value = post_storage.and_then(|s| s.get(slot)).copied();

            if pre_value != post_value {
                let before = if pre_value == Some(B256::ZERO) {
                    None
                } else {
                    pre_value
                };
                let after = if post_value == Some(B256::ZERO) {
                    None
                } else {
                    post_value
                };

                state_diff.add_change(*address, *slot, before, after);
            }
        }
    }

    Ok(state_diff)
}

pub(crate) async fn backfill_rpc(
    untraced: &[Transaction],
    provider: &Arc<GenericProvider>,
    txs: &sqlx::SqlitePool,
) -> Result<()> {
    let (from, to) = block_bounds(untraced);
    let blocks = Block::query_where(&format!("block_number BETWEEN {from} AND {to}"), txs).await?;
    let coinbase_by_block: HashMap<u64, Address> =
        blocks.iter().map(|b| (b.block_number, b.miner)).collect();

    let to_trace: Vec<(FixedBytes<32>, Address)> = untraced
        .iter()
        .filter_map(|tx| {
            coinbase_by_block
                .get(&tx.block_number)
                .map(|cb| (tx.tx_hash, *cb))
        })
        .collect();

    let total = to_trace.len();
    info!("Tracing coinbase payments for {} txs (rpc)", total);

    for (n, (tx_hash, coinbase)) in to_trace.into_iter().enumerate() {
        debug!("Tracing {}/{} (0x{})", n + 1, total, hex::encode(tx_hash));
        match rpc_tx_calls(tx_hash, provider).await {
            Ok(frames) => {
                let traces: Vec<TraceData> = frames.into_iter().map(Into::into).collect();
                let value = find_coinbase_transfer(coinbase, traces);
                // Commit each tx on its own so an interrupt keeps prior progress.
                Transaction::update_coinbase_transfer(tx_hash, value, txs).await?;
                info!(
                    "Committed coinbase_transfer {}/{} (0x{})",
                    n + 1,
                    total,
                    hex::encode(tx_hash)
                );
            }
            Err(e) => {
                warn!(
                    "coinbase_transfer trace failed for 0x{}: {}",
                    hex::encode(tx_hash),
                    e
                );
            }
        }
    }

    Ok(())
}

fn block_bounds(txs: &[Transaction]) -> (u64, u64) {
    let mut min = u64::MAX;
    let mut max = 0;
    for tx in txs {
        min = min.min(tx.block_number);
        max = max.max(tx.block_number);
    }
    (min, max)
}
