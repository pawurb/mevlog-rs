//! In-process command logic shared by the CLI (`bin/cmd/*`) and the TUI.
//!
//! Each function performs the work of one CLI subcommand and **returns a value**
//! (no printing). The clap layer is a thin wrapper that parses args, calls the
//! matching function here, then formats and prints. The TUI calls these
//! functions directly, receiving concrete typed values with no JSON round-trip.
//!
//! SQL-backed commands (`query`, `block`, `block_txs`, `block_logs`, `tx`,
//! `tx_logs`) return a [`QueryOutcome`](crate::models::json::query_response::QueryOutcome)
//! carrying the generic `columns + rows` plus envelope metadata. The
//! TUI-consumed ones (`block_txs`, `block_logs`, `tx`) also expose typed
//! wrappers that deserialize the rows into concrete types.

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
pub mod query;
pub mod state_diff;
pub mod tx;
pub mod tx_logs;
pub mod update_db;
