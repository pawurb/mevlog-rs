use std::{collections::HashSet, mem, sync::Arc};

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

use crate::{GenericProvider, models::mev_opcode::MEVOpcode};

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

    let frame = match trace {
        GethTrace::CallTracer(frame) => frame,
        _ => unreachable!(),
    };
    let mut all_calls = Vec::new();

    collect_calls(frame, &mut all_calls);
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

    let mut opcodes = Vec::with_capacity(struct_logs.len());

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

fn collect_calls(mut frame: CallFrame, result: &mut Vec<CallFrame>) {
    let calls = mem::take(&mut frame.calls);
    result.push(frame);

    for call in calls {
        collect_calls(call, result);
    }
}
