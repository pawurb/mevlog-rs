use std::path::PathBuf;

use eyre::Result;
use mevlog::{
    misc::shared_init::OutputFormat,
    models::json::query_response::{
        HtmlMeta, QueryOutcome, content_hash, format_duration, rows_to_csv, rows_to_html,
        rows_to_table, serialize_query_response,
    },
};

/// Destination controls for `--format html`, populated from the global
/// `--html-path` / `--html-filename` flags.
#[derive(Debug, Default, Clone)]
pub(crate) struct HtmlOpts {
    pub path: Option<PathBuf>,
    pub filename: Option<String>,
}

/// Renders a SQL-backed command's [`QueryOutcome`] for the chosen output format:
/// CSV/table emit only the result rows, JSON wraps them in the response
/// envelope, HTML writes a self-contained page to disk and prints its path.
pub(crate) fn print_query_outcome(
    outcome: QueryOutcome,
    format: OutputFormat,
    html: &HtmlOpts,
) -> Result<()> {
    match format {
        OutputFormat::Csv => print!("{}", rows_to_csv(&outcome.columns, &outcome.rows)?),
        OutputFormat::Table => print!("{}", rows_to_table(&outcome.columns, &outcome.rows)),
        OutputFormat::Html => write_html_output(&outcome, html)?,
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

/// Renders the outcome as a standalone HTML page and writes it to
/// `<html.path or cwd>/<html.filename or mevlog-<content-hash>.html>`, printing
/// the resulting absolute path.
fn write_html_output(outcome: &QueryOutcome, html: &HtmlOpts) -> Result<()> {
    let duration = format_duration(outcome.duration_ns);
    let meta = HtmlMeta {
        chain_name: &outcome.chain.name,
        chain_id: outcome.chain.chain_id,
        blocks: outcome.query.blocks.as_deref(),
        sql: outcome.query.sql.as_deref(),
        row_count: outcome.rows.len(),
        duration: &duration,
    };
    let page = rows_to_html(&outcome.columns, &outcome.rows, &meta);

    let filename = match &html.filename {
        Some(name) if name.ends_with(".html") => name.clone(),
        Some(name) => format!("{name}.html"),
        None => format!(
            "mevlog-{}.html",
            content_hash(
                &outcome.chain,
                &outcome.query,
                &outcome.columns,
                &outcome.rows,
            )
        ),
    };

    let dir = html.path.clone().unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);
    std::fs::write(&path, page)?;

    let abs = std::fs::canonicalize(&path).unwrap_or(path);
    println!("{}", abs.display());
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
