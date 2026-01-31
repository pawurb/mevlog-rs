use std::{collections::HashSet, sync::Arc};

use alloy::{
    primitives::{B256, TxHash},
    providers::ext::DebugApi,
    rpc::types::trace::geth::{
        CallFrame, DiffMode, GethDebugBuiltInTracerType, GethDebugTracerType,
        GethDebugTracingOptions, GethTrace, PreStateConfig, PreStateFrame,
    },
};
use eyre::Result;
use revm::primitives::Address;

use crate::{
    GenericProvider,
    models::{mev_opcode::MEVOpcode, mev_state_diff::MEVStateDiff},
};

#[hotpath::measure(log = true)]
pub async fn rpc_tx_calls(
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

#[hotpath::measure(log = true)]
pub async fn rpc_touching_accounts(
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

    let diff_traces = match trace {
        GethTrace::PreStateTracer(PreStateFrame::Diff(DiffMode { post: frame, .. })) => frame,
        _ => unreachable!(),
    };

    Ok(diff_traces.keys().copied().collect())
}

#[hotpath::measure(log = true)]
pub async fn rpc_tx_opcodes(
    tx_hash: TxHash,
    provider: &Arc<GenericProvider>,
) -> Result<Vec<MEVOpcode>> {
    let tracing_opts = GethDebugTracingOptions::default();

    let trace = match provider
        .debug_trace_transaction(tx_hash, tracing_opts)
        .await
    {
        Ok(trace) => trace,
        Err(e) => {
            tracing::error!("Error tracing tx opcodes: {}", e);
            eyre::bail!("Error tracing tx opcodes: {}", e);
        }
    };

    let struct_logs = match trace {
        GethTrace::Default(default_frame) => default_frame.struct_logs,
        _ => {
            tracing::warn!("Unexpected trace type for opcode tracing");
            return Ok(vec![]);
        }
    };

    let mut opcodes = Vec::new();

    for log in struct_logs {
        opcodes.push(MEVOpcode::new(
            log.pc,
            log.op.to_string(),
            log.gas_cost,
            log.gas,
        ));
    }

    Ok(opcodes)
}

fn collect_calls(frame: &CallFrame, result: &mut Vec<CallFrame>) {
    result.push(frame.clone());

    for call in &frame.calls {
        collect_calls(call, result);
    }
}

#[hotpath::measure(log = true)]
pub async fn rpc_tx_state_diff(
    tx_hash: TxHash,
    provider: &Arc<GenericProvider>,
) -> Result<MEVStateDiff> {
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
            return Ok(MEVStateDiff::new());
        }
    };

    let (pre_state, post_state) = match trace {
        GethTrace::PreStateTracer(PreStateFrame::Diff(DiffMode { pre, post })) => (pre, post),
        _ => {
            tracing::warn!("Unexpected trace type for state diff tracing");
            return Ok(MEVStateDiff::new());
        }
    };

    let mut state_diff = MEVStateDiff::new();

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
