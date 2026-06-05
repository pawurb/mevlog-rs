use std::{collections::HashMap, sync::Arc};

use eyre::{Result, bail};
use revm::primitives::{Address, FixedBytes, U256};
use tracing::{debug, info, warn};

use crate::{
    GenericProvider,
    db::txs::models::{block::Block, transaction::Transaction},
    misc::{
        coinbase_bribe::{TraceData, find_coinbase_transfer},
        rpc_tracing::rpc_tx_calls,
        shared_init::TraceMode,
    },
};

/// Traces every stored tx in `from..=to` whose `coinbase_transfer` is still NULL
/// and backfills it with the direct ETH it paid to its block's coinbase.
///
/// A traced tx with no coinbase payment is stored as `Some(0)`, so a remaining
/// NULL always means "never traced" (block beneficiary unknown, or the trace
/// failed). Covers both freshly-indexed blocks and blocks indexed earlier
/// without `--evm-trace`.
pub async fn backfill_coinbase_transfers(
    from: u64,
    to: u64,
    mode: &TraceMode,
    provider: &Arc<GenericProvider>,
    txs: &sqlx::SqlitePool,
) -> Result<()> {
    match mode {
        TraceMode::Revm => bail!(
            "--evm-trace revm is not yet supported for coinbase_transfer indexing; \
             use --evm-trace rpc"
        ),
        TraceMode::RPC => {}
    }

    let untraced = Transaction::query_where(
        &format!("coinbase_transfer IS NULL AND block_number BETWEEN {from} AND {to}"),
        txs,
    )
    .await?;
    if untraced.is_empty() {
        return Ok(());
    }

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
    info!("Tracing coinbase payments for {} txs", total);

    let mut updates: Vec<(FixedBytes<32>, U256)> = Vec::new();
    for (n, (tx_hash, coinbase)) in to_trace.into_iter().enumerate() {
        debug!("Tracing {}/{} (0x{})", n + 1, total, hex::encode(tx_hash));
        match rpc_tx_calls(tx_hash, provider).await {
            Ok(frames) => {
                let traces: Vec<TraceData> = frames.into_iter().map(Into::into).collect();
                updates.push((tx_hash, find_coinbase_transfer(coinbase, traces)));
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

    Transaction::update_coinbase_transfers(&updates, txs).await?;

    Ok(())
}
