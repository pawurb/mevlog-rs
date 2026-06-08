use alloy::primitives::TxHash;
use eyre::{Result, bail};
use revm::primitives::Address;

use crate::{
    db::sigs::models::chain::Chain,
    misc::{
        shared_init::{ConnOpts, TraceMode, resolve_conn},
        tx_tracing::affected_addresses_for_tx,
    },
    models::evm_chain::EVMChain,
};

/// Lists the addresses affected by a transaction via the selected backend.
pub async fn affected_addresses(
    tx_hash: TxHash,
    evm_trace: Option<&TraceMode>,
    conn_opts: &ConnOpts,
) -> Result<Vec<Address>> {
    let Some(mode) = evm_trace else {
        bail!("--evm-trace [rpc|revm] must be specified")
    };

    let conn = resolve_conn(conn_opts).await?;
    let chain = EVMChain::new(Chain::unknown(conn.chain_id as i64), conn.rpc_url.clone())?;

    affected_addresses_for_tx(tx_hash, mode, &conn.provider, &chain, &conn.rpc_url).await
}
