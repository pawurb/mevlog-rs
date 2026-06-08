use alloy::primitives::TxHash;
use eyre::{Result, bail};

use crate::{
    db::sigs::models::chain::Chain,
    misc::{
        shared_init::{ConnOpts, TraceMode, resolve_conn},
        tx_tracing::{CoinbaseTransfer, coinbase_transfer_for_tx},
    },
    models::evm_chain::EVMChain,
};

/// Computes a transaction's direct ETH payment to its block's coinbase.
pub async fn coinbase_transfer(
    tx_hash: TxHash,
    evm_trace: Option<&TraceMode>,
    conn_opts: &ConnOpts,
) -> Result<CoinbaseTransfer> {
    let Some(mode) = evm_trace else {
        bail!("--evm-trace [rpc|revm] must be specified")
    };

    let conn = resolve_conn(conn_opts).await?;
    let chain = EVMChain::new(Chain::unknown(conn.chain_id as i64), conn.rpc_url.clone())?;

    coinbase_transfer_for_tx(tx_hash, mode, &conn.provider, &chain, &conn.rpc_url).await
}
