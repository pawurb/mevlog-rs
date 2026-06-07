use eyre::Result;
use mevlog::{
    misc::shared_init::OutputFormat,
    models::json::query_response::{
        QueryOutcome, rows_to_csv, rows_to_table, serialize_query_response,
    },
};

/// Renders a SQL-backed command's [`QueryOutcome`] for the chosen output format:
/// CSV/table emit only the result rows, JSON wraps them in the response envelope.
pub fn print_query_outcome(outcome: QueryOutcome, format: OutputFormat) -> Result<()> {
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

pub mod affected_addresses;
pub mod block;
pub mod block_logs;
pub mod block_txs;
pub mod chain_info;
pub mod chains;
pub mod coinbase_transfer;
pub mod debug_available;
pub mod ens_lookup;
pub mod ens_resolve;
pub mod evm_traces;
pub mod index;
pub mod query;
pub mod state_diff;
#[cfg(feature = "tui")]
pub mod tui;
pub mod tx;
pub mod tx_logs;
pub mod update_db;

#[cfg(feature = "mcp")]
pub mod mcp;
#[cfg(feature = "seed-db")]
pub mod seed_db;
