use std::{collections::HashSet, sync::Arc};

use alloy::{
    eips::BlockNumberOrTag, network::ReceiptResponse, primitives::TxHash, providers::Provider,
    rpc::types::trace::parity::Action,
};
use eyre::Result;
use revm::primitives::{Address, TxKind, U256};
use sqlx::SqlitePool;

use crate::{
    GenericProvider,
    db::txs::models::transaction::{Transaction, extract_signature},
    misc::{
        coinbase_bribe::{TraceData, find_coinbase_transfer},
        revm_tracing::{
            backfill_revm, revm_affected_addresses_for_tx, revm_block_traced_calls,
            revm_calls_for_tx, revm_state_diff_for_tx,
        },
        rpc_tracing::{backfill_rpc, rpc_affected_addresses, rpc_tx_calls, rpc_tx_state_diff},
        shared_init::TraceMode,
        utils::{ETH_TRANSFER, wei_to_eth},
    },
    models::{evm_chain::EVMChain, mev_transaction::CallExtract, state_diff::StateDiff},
};

/// Direct ETH a single tx paid to its block's coinbase (miner/validator).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CoinbaseTransfer {
    pub tx_hash: TxHash,
    pub coinbase: Address,
    /// Amount in wei, rendered as a decimal string to stay JSON-safe.
    #[serde(serialize_with = "serialize_u256_dec")]
    pub amount_wei: U256,
    pub amount_eth: f64,
}

fn serialize_u256_dec<S: serde::Serializer>(v: &U256, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&v.to_string())
}

impl std::fmt::Display for CoinbaseTransfer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "tx 0x{} paid {} ETH ({} wei) to coinbase {}",
            hex::encode(self.tx_hash),
            self.amount_eth,
            self.amount_wei,
            self.coinbase,
        )
    }
}

/// Computes the direct coinbase transfer of a single tx identified by its hash.
///
/// Traces the tx's calls (over RPC or local Revm replay), looks up its block's
/// beneficiary, and returns the ETH sent straight to that address (`0` if none).
pub async fn coinbase_transfer_for_tx(
    tx_hash: TxHash,
    mode: &TraceMode,
    provider: &Arc<GenericProvider>,
    chain: &EVMChain,
    rpc_url: &str,
) -> Result<CoinbaseTransfer> {
    let (coinbase, traces): (Address, Vec<TraceData>) = match mode {
        TraceMode::RPC => {
            let coinbase = block_coinbase_for_tx(tx_hash, provider).await?;
            let frames = rpc_tx_calls(tx_hash, provider).await?;
            (coinbase, frames.into_iter().map(Into::into).collect())
        }
        TraceMode::Revm => {
            let block_number = tx_block_number(tx_hash, provider).await?;
            let targets = HashSet::from([tx_hash]);
            let (ctx, mut traced) =
                revm_block_traced_calls(block_number, &targets, provider, rpc_url, chain, None)
                    .await?;
            let calls = traced.remove(&tx_hash).unwrap_or_default();
            (ctx.coinbase, calls.into_iter().map(Into::into).collect())
        }
    };

    let amount_wei = find_coinbase_transfer(coinbase, traces);

    Ok(CoinbaseTransfer {
        tx_hash,
        coinbase,
        amount_wei,
        amount_eth: wei_to_eth(amount_wei),
    })
}

/// Addresses affected by a single tx according to the selected trace backend.
pub async fn affected_addresses_for_tx(
    tx_hash: TxHash,
    mode: &TraceMode,
    provider: &Arc<GenericProvider>,
    chain: &EVMChain,
    rpc_url: &str,
) -> Result<Vec<Address>> {
    let addresses = match mode {
        TraceMode::RPC => rpc_affected_addresses(tx_hash, provider).await?,
        TraceMode::Revm => {
            let block_number = tx_block_number(tx_hash, provider).await?;
            revm_affected_addresses_for_tx(tx_hash, block_number, provider, rpc_url, chain).await?
        }
    };

    let mut addresses: Vec<_> = addresses.into_iter().collect();
    addresses.sort();
    Ok(addresses)
}

/// Storage state diff produced by a single tx according to the selected backend.
pub async fn state_diff_for_tx(
    tx_hash: TxHash,
    mode: &TraceMode,
    provider: &Arc<GenericProvider>,
    chain: &EVMChain,
    rpc_url: &str,
) -> Result<StateDiff> {
    Ok(match mode {
        TraceMode::RPC => rpc_tx_state_diff(tx_hash, provider).await?,
        TraceMode::Revm => {
            let block_number = tx_block_number(tx_hash, provider).await?;
            revm_state_diff_for_tx(tx_hash, block_number, provider, rpc_url, chain).await?
        }
    })
}

/// Decoded call traces of a single tx according to the selected backend.
pub async fn calls_for_tx(
    tx_hash: TxHash,
    mode: &TraceMode,
    provider: &Arc<GenericProvider>,
    chain: &EVMChain,
    rpc_url: &str,
    sqlite: &SqlitePool,
) -> Result<Vec<CallExtract>> {
    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction 0x{} not found", hex::encode(tx_hash)))?;
    let tx_index = receipt
        .transaction_index()
        .ok_or_else(|| eyre::eyre!("Transaction 0x{} is not mined yet", hex::encode(tx_hash)))?;

    let mut calls = Vec::new();
    match mode {
        TraceMode::RPC => {
            for call in rpc_tx_calls(tx_hash, provider).await? {
                let Some(to) = call.to else { continue };
                let (signature_hash, signature) =
                    extract_signature(Some(&call.input), tx_index, Some(TxKind::Call(to)), sqlite)
                        .await?;
                calls.push(CallExtract {
                    from: call.from,
                    to,
                    signature: signature.unwrap_or_else(|| ETH_TRANSFER.to_string()),
                    signature_hash: signature_hash.map(|h| format!("0x{}", hex::encode(h))),
                });
            }
        }
        TraceMode::Revm => {
            let block_number = receipt.block_number().ok_or_else(|| {
                eyre::eyre!("Transaction 0x{} is not mined yet", hex::encode(tx_hash))
            })?;
            for trace in revm_calls_for_tx(tx_hash, block_number, provider, rpc_url, chain).await? {
                let Action::Call(action) = &trace.action else {
                    continue;
                };
                let (signature_hash, signature) = extract_signature(
                    Some(&action.input),
                    tx_index,
                    Some(TxKind::Call(action.to)),
                    sqlite,
                )
                .await?;
                calls.push(CallExtract {
                    from: action.from,
                    to: action.to,
                    signature: signature.unwrap_or_else(|| ETH_TRANSFER.to_string()),
                    signature_hash: signature_hash.map(|h| format!("0x{}", hex::encode(h))),
                });
            }
        }
    }

    Ok(calls)
}

/// Resolves the beneficiary (coinbase) of the block that mined `tx_hash`.
async fn block_coinbase_for_tx(
    tx_hash: TxHash,
    provider: &Arc<GenericProvider>,
) -> Result<Address> {
    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction 0x{} not found", hex::encode(tx_hash)))?;

    let block_number = receipt
        .block_number()
        .ok_or_else(|| eyre::eyre!("Transaction 0x{} is not mined yet", hex::encode(tx_hash)))?;

    let block = provider
        .get_block_by_number(BlockNumberOrTag::Number(block_number))
        .await?
        .ok_or_else(|| eyre::eyre!("Block {} not found", block_number))?;

    Ok(block.header.beneficiary)
}

/// Resolves the number of the block that mined `tx_hash` from its receipt.
async fn tx_block_number(tx_hash: TxHash, provider: &Arc<GenericProvider>) -> Result<u64> {
    let receipt = provider
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction 0x{} not found", hex::encode(tx_hash)))?;

    receipt
        .block_number()
        .ok_or_else(|| eyre::eyre!("Transaction 0x{} is not mined yet", hex::encode(tx_hash)))
}

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
    chain: &EVMChain,
    rpc_url: &str,
    txs: &sqlx::SqlitePool,
) -> Result<()> {
    let untraced = Transaction::query_where(
        &format!("coinbase_transfer IS NULL AND block_number BETWEEN {from} AND {to}"),
        txs,
    )
    .await?;
    if untraced.is_empty() {
        return Ok(());
    }

    match mode {
        TraceMode::RPC => backfill_rpc(&untraced, provider, txs).await?,
        TraceMode::Revm => backfill_revm(&untraced, provider, chain, rpc_url, txs).await?,
    }

    Ok(())
}
