//! In-process command logic shared by the CLI (`bin/cmd/*`) and the TUI. Each
//! function performs the work of one subcommand and returns a value rather than
//! printing; the clap layer parses args, calls it, then formats the result.
//!
//! SQL-backed commands (`query`, `block`, `block_txs`, `block_logs`, `tx`,
//! `tx_logs`) return a [`QueryOutcome`](crate::models::json::query_response::QueryOutcome)
//! of generic `columns + rows` plus envelope metadata. The TUI-consumed ones
//! (`block_txs`, `block_logs`, `tx`) also expose typed wrappers that deserialize
//! the rows into concrete types.

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
