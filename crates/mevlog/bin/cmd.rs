use eyre::Result;
use mevlog::{
    misc::shared_init::OutputFormat,
    models::json::query_response::{
        QueryOutcome, rows_to_csv, rows_to_table, serialize_query_response,
    },
};

/// Renders a SQL-backed command's [`QueryOutcome`] for the chosen output format:
/// CSV/table emit only the result rows, JSON wraps them in the response envelope.
pub(crate) fn print_query_outcome(outcome: QueryOutcome, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Csv => print!("{}", rows_to_csv(&outcome.columns, &outcome.rows)?),
        OutputFormat::Table => print!("{}", rows_to_table(&outcome.columns, &outcome.rows)),
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let pretty = matches!(format, OutputFormat::JsonPretty);
            let output = serialize_query_response(
                outcome.rows,
                pretty,
                outcome.chain,
                outcome.duration_ns,
                outcome.cached_blocks,
                outcome.new_blocks,
                outcome.query,
            )?;
            println!("{output}");
        }
    }
    Ok(())
}

pub(crate) mod affected_addresses;
pub(crate) mod block;
pub(crate) mod block_logs;
pub(crate) mod block_txs;
pub(crate) mod chain_info;
pub(crate) mod chains;
pub(crate) mod coinbase_transfer;
pub(crate) mod db_info;
pub(crate) mod debug_available;
pub(crate) mod ens_lookup;
pub(crate) mod ens_resolve;
pub(crate) mod evm_traces;
pub(crate) mod index;
pub(crate) mod purge_db;
pub(crate) mod query;
pub(crate) mod reindex;
pub(crate) mod state_diff;
#[cfg(feature = "tui")]
pub(crate) mod tui;
pub(crate) mod tx;
pub(crate) mod tx_logs;
pub(crate) mod update_custom_tables;
pub(crate) mod update_sigs_db;

#[cfg(feature = "mcp")]
pub(crate) mod mcp;
#[cfg(feature = "seed-db")]
pub(crate) mod seed_db;
