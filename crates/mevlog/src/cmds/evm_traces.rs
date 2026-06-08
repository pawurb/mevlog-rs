use alloy::primitives::TxHash;
use eyre::{Result, bail};

use crate::{
    misc::{
        shared_init::{ConnOpts, TraceMode, init_deps},
        tx_tracing::calls_for_tx,
    },
    models::call_extract::CallExtract,
};

/// Extracts a transaction's decoded call traces via the selected backend.
pub async fn evm_traces(
    tx_hash: TxHash,
    evm_trace: Option<&TraceMode>,
    conn_opts: &ConnOpts,
) -> Result<Vec<CallExtract>> {
    let Some(mode) = evm_trace else {
        bail!("--evm-trace [rpc|revm] must be specified")
    };

    let deps = init_deps(conn_opts).await?;

    calls_for_tx(
        tx_hash,
        mode,
        &deps.provider,
        deps.chain.as_ref(),
        &deps.rpc_url,
        &deps.sqlite,
    )
    .await
}
