//! Canonical `SELECT`s that render the txs store as [`TransactionJson`] /
//! [`LogJson`], with ETH/gwei/USD/decimal columns computed by the `evm-sqlite`
//! functions. The USD columns embed the `{NATIVE_TOKEN_PRICE()}` macro, expanded
//! by [`substitute_sql_macros`] before the query runs.
//!
//! [`TransactionJson`]: crate::models::json::transaction_json::TransactionJson
//! [`LogJson`]: crate::models::json::log_json::LogJson
//! [`substitute_sql_macros`]: crate::misc::sql_macros::substitute_sql_macros

use crate::misc::{sql_macros::NATIVE_TOKEN_PRICE_MACRO, utils::ETH_TRANSFER};

/// Canonical transactions `SELECT` for the given `WHERE` clause, projecting the
/// columns and display strings of [`TransactionJson`] (no `logs`).
///
/// `txcost = gas_used * effective_gas_price` and
/// `fullcost = txcost + coinbase_transfer` are computed once in an inner query;
/// `fullcost` is `NULL` when `coinbase_transfer` is `NULL` (untraced tx).
pub fn tx_display_query(where_sql: &str) -> String {
    let price = NATIVE_TOKEN_PRICE_MACRO;
    format!(
        "SELECT \
            block_number, \
            tx_index, \
            tx_hash, \
            from_address AS \"from\", \
            to_address AS \"to\", \
            nonce, \
            COALESCE(signature, '{ETH_TRANSFER}') AS signature, \
            signature_hash, \
            success, \
            u256_to_dec(value) AS value, \
            format_ether(value) AS display_value, \
            gas_used, \
            effective_gas_price AS gas_price, \
            format_gwei(effective_gas_price) AS display_gas_price, \
            u256_to_dec(txcost) AS tx_cost, \
            format_ether(txcost) AS display_tx_cost, \
            format_usd(txcost, {price}) AS display_tx_cost_usd, \
            u256_to_dec(coinbase_transfer) AS coinbase_transfer, \
            format_ether(coinbase_transfer) AS display_coinbase_transfer, \
            format_usd(coinbase_transfer, {price}) AS display_coinbase_transfer_usd, \
            u256_to_dec(fullcost) AS full_tx_cost, \
            format_ether(fullcost) AS display_full_tx_cost, \
            format_usd(fullcost, {price}) AS display_full_tx_cost_usd \
         FROM ( \
            SELECT *, \
                u256_mul(gas_used, effective_gas_price) AS txcost, \
                u256_add(u256_mul(gas_used, effective_gas_price), coinbase_transfer) AS fullcost \
            FROM transactions \
            WHERE {where_sql} \
         ) \
         ORDER BY block_number DESC, tx_index ASC"
    )
}

/// Canonical logs `SELECT` for the given `WHERE` clause, projecting the columns
/// of [`LogJson`]. `topic0..topic3` are returned as separate columns; the caller
/// folds the non-null ones into the `topics` array.
pub fn logs_display_query(where_sql: &str) -> String {
    format!(
        "SELECT \
            log_index, \
            address, \
            signature, \
            topic0, topic1, topic2, topic3, \
            data, \
            u256_to_dec(erc20_amount) AS erc20_amount \
         FROM logs \
         WHERE {where_sql} \
         ORDER BY log_index ASC"
    )
}
