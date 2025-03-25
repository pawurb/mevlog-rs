use std::{collections::HashSet, sync::Arc};

use alloy::{
    primitives::TxHash,
    providers::ext::DebugApi,
    rpc::types::trace::geth::{
        CallFrame, DiffMode, GethDebugBuiltInTracerType, GethDebugTracerType,
        GethDebugTracingOptions, GethTrace, PreStateConfig, PreStateFrame,
    },
};
use eyre::Result;
use revm::primitives::Address;

use crate::GenericProvider;

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

fn collect_calls(frame: &CallFrame, result: &mut Vec<CallFrame>) {
    result.push(frame.clone());

    for call in &frame.calls {
        collect_calls(call, result);
    }
}
