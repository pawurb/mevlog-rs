use std::time::Instant;

use alloy::{network::ReceiptResponse, primitives::TxHash, providers::Provider};
use eyre::{Result, bail};

use crate::{
    ChainInfoNoRpcsJson,
    db::txs::{
        display_sql::tx_display_query, indexing::index_block_range,
        models::transaction::Transaction, raw_query::run_raw_query_async,
    },
    misc::{
        args_parsing::get_latest_block,
        shared_init::{ConnOpts, CryoOpts, TraceMode, init_deps},
        sql_macros::{NATIVE_TOKEN_PRICE_MACRO, substitute_sql_macros},
        tx_tracing::coinbase_transfer_for_tx,
        utils::get_native_token_price,
    },
    models::json::{
        query_response::{QueryOutcome, QueryParams},
        transaction_json::TransactionJson,
    },
};

/// Indexes the block holding `tx_hash` (optionally backfilling its coinbase
/// payment via tracing) and returns the single transaction, shaped for display.
pub async fn tx(
    tx_hash: TxHash,
    evm_trace: Option<&TraceMode>,
    native_token_price: Option<f64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<QueryOutcome> {
    let deps = init_deps(conn_opts).await?;

    let native_token_price =
        get_native_token_price(&deps.chain, &deps.provider, native_token_price).await?;

    let latest_block = Some(get_latest_block(&deps.provider, None).await?);

    let receipt = deps
        .provider
        .get_transaction_receipt(tx_hash)
        .await?
        .ok_or_else(|| eyre::eyre!("Transaction {} not found", tx_hash))?;
    let block_number = receipt
        .block_number()
        .ok_or_else(|| eyre::eyre!("Transaction {} is not mined yet", tx_hash))?;

    let start_time = Instant::now();

    let (cached_blocks, new_blocks) =
        index_block_range(block_number, block_number, 1, &deps, cryo_opts).await?;

    if let Some(mode) = evm_trace {
        let coinbase_transfer =
            coinbase_transfer_for_tx(tx_hash, mode, &deps.provider, &deps.chain, &deps.rpc_url)
                .await?;
        Transaction::update_coinbase_transfer(tx_hash, coinbase_transfer.amount_wei, &deps.txs)
            .await?;
    }

    let mut chain_info = ChainInfoNoRpcsJson::from_evm_chain(&deps.chain);
    chain_info.native_token_price = native_token_price;
    let duration_ns = start_time.elapsed().as_nanos() as u64;

    let sql = tx_display_query(&format!("tx_hash = X'{}'", hex::encode(tx_hash)));
    // Without a price the macro can't resolve, so substitute NULL; the USD
    // columns then render as null rather than erroring.
    let sql = if native_token_price.is_some() {
        substitute_sql_macros(
            &sql,
            &deps.provider,
            deps.chain.chain_id,
            native_token_price,
            None,
        )
        .await?
    } else {
        sql.replace(NATIVE_TOKEN_PRICE_MACRO, "NULL")
    };

    let result = run_raw_query_async(
        sql.clone(),
        deps.txs_read_path.clone(),
        None,
        None,
        deps.custom_table_names(),
    )
    .await?;
    if result.rows.is_empty() {
        bail!("Transaction {} not found in local store", tx_hash);
    }

    Ok(QueryOutcome {
        columns: result.columns,
        rows: result.rows,
        cached_blocks,
        new_blocks,
        latest_block,
        duration_ns,
        chain: chain_info,
        query: QueryParams {
            blocks: Some(block_number.to_string()),
            sql: Some(sql),
            evm_trace: evm_trace.cloned(),
        },
    })
}

/// Typed convenience wrapper for in-process callers (the TUI): returns the
/// single transaction as a concrete [`TransactionJson`].
pub async fn tx_typed(
    tx_hash: TxHash,
    evm_trace: Option<&TraceMode>,
    native_token_price: Option<f64>,
    conn_opts: &ConnOpts,
    cryo_opts: &CryoOpts,
) -> Result<TransactionJson> {
    let rows: Vec<TransactionJson> =
        tx(tx_hash, evm_trace, native_token_price, conn_opts, cryo_opts)
            .await?
            .rows_as()?;
    rows.into_iter()
        .next()
        .ok_or_else(|| eyre::eyre!("Transaction {} not found in local store", tx_hash))
}
