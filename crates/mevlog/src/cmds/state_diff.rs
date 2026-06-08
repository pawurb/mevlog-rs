use alloy::primitives::TxHash;
use eyre::{Result, bail};

use crate::{
    db::sigs::models::chain::Chain,
    misc::{
        shared_init::{ConnOpts, TraceMode, resolve_conn},
        tx_tracing::state_diff_for_tx,
    },
    models::{evm_chain::EVMChain, state_diff::StateDiff},
};

/// Computes the storage state diff produced by a transaction via the selected
/// backend. The caller renders it (`StateDiffJson::from(&diff)` for JSON).
pub async fn state_diff(
    tx_hash: TxHash,
    evm_trace: Option<&TraceMode>,
    conn_opts: &ConnOpts,
) -> Result<StateDiff> {
    let Some(mode) = evm_trace else {
        bail!("--evm-trace [rpc|revm] must be specified")
    };

    let conn = resolve_conn(conn_opts).await?;
    let chain = EVMChain::new(Chain::unknown(conn.chain_id as i64), conn.rpc_url.clone())?;

    state_diff_for_tx(tx_hash, mode, &conn.provider, &chain, &conn.rpc_url).await
}
