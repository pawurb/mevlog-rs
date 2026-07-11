use std::path::PathBuf;

use eyre::Result;
use mevlog::{
    misc::{config::Config, ipfs, shared_init::OutputFormat},
    models::json::query_response::{
        HtmlMeta, QueryOutcome, content_hash, format_duration, generated_at_utc, rows_to_csv,
        rows_to_html, rows_to_table, serialize_query_response,
    },
};

/// Destination controls for `--format html`, populated from the global
/// `--html-path` / `--html-filename` flags.
#[derive(Debug, Default, Clone)]
pub(crate) struct HtmlOpts {
    pub path: Option<PathBuf>,
    pub filename: Option<String>,
}

/// Rendering controls shared by the SQL-backed query commands, populated from
/// the global `--format` / `--html-*` / `--ipfs` / `--desc` flags.
#[derive(Debug, Clone)]
pub(crate) struct RenderOpts {
    pub format: OutputFormat,
    pub html: HtmlOpts,
    pub ipfs: bool,
    pub desc: Option<String>,
}

/// Renders a SQL-backed command's [`QueryOutcome`] for the chosen output format
/// (CSV/table emit the rows, JSON wraps them in the response envelope, HTML is a
/// self-contained page). `--desc` becomes the envelope's `description` field, a
/// line above the table output and the HTML page title (CSV stays bare - a
/// description line would corrupt parsers). Without `--ipfs` the result is
/// printed (HTML is written to a file and its path printed); with `--ipfs` the
/// rendered bytes are uploaded to IPFS and a CID + gateway URL is printed
/// instead.
pub(crate) async fn print_query_outcome(outcome: QueryOutcome, render: &RenderOpts) -> Result<()> {
    let format = render.format.clone();
    let desc = render.desc.as_deref();

    // The content hash names the html/ipfs artifact; skip it on the hot path
    // (plain stdout formats) where it is never used.
    let hash = if render.ipfs || matches!(format, OutputFormat::Html) {
        content_hash(
            &outcome.chain,
            &outcome.query,
            desc,
            &outcome.columns,
            &outcome.rows,
        )
    } else {
        String::new()
    };

    let (body, content_type, ext) = match format {
        OutputFormat::Csv => (
            rows_to_csv(&outcome.columns, &outcome.rows)?,
            "text/csv",
            "csv",
        ),
        OutputFormat::Table => {
            let table = rows_to_table(&outcome.columns, &outcome.rows);
            let body = match desc {
                Some(desc) => format!("{desc}\n{table}"),
                None => table,
            };
            let body = format!("{body}\ngenerated_at: {}", generated_at_utc());
            (body, "text/plain", "txt")
        }
        OutputFormat::Html => {
            let duration = format_duration(outcome.duration_ns);
            let generated_at = generated_at_utc();
            let meta = HtmlMeta {
                chain_name: &outcome.chain.name,
                chain_id: outcome.chain.chain_id,
                blocks: outcome.query.blocks.as_deref(),
                sql: outcome.query.sql.as_deref(),
                description: desc,
                row_count: outcome.rows.len(),
                duration: &duration,
                generated_at: &generated_at,
            };
            (
                rows_to_html(&outcome.columns, &outcome.rows, &meta),
                "text/html",
                "html",
            )
        }
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
                render.desc.clone(),
            )?;
            (output, "application/json", "json")
        }
    };

    if render.ipfs {
        return upload_to_ipfs(body, content_type, &hash, ext, format).await;
    }

    match format {
        OutputFormat::Html => write_html_file(&body, &hash, &render.html)?,
        OutputFormat::Json | OutputFormat::JsonPretty => println!("{body}"),
        OutputFormat::Csv | OutputFormat::Table => print!("{body}"),
    }
    Ok(())
}

/// Writes the rendered HTML page to
/// `<html.path or cwd>/<html.filename or mevlog-<hash>.html>` and prints the
/// resulting absolute path.
fn write_html_file(page: &str, hash: &str, html: &HtmlOpts) -> Result<()> {
    let filename = match &html.filename {
        Some(name) if name.ends_with(".html") => name.clone(),
        Some(name) => format!("{name}.html"),
        None => format!("mevlog-{hash}.html"),
    };

    let dir = html.path.clone().unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);
    std::fs::write(&path, page)?;

    let abs = std::fs::canonicalize(&path).unwrap_or(path);
    println!("{}", abs.display());
    Ok(())
}

/// Uploads the rendered output to IPFS and prints the CID + gateway URL (as JSON
/// for the json/json-pretty formats, as a human summary otherwise).
async fn upload_to_ipfs(
    body: String,
    content_type: &str,
    hash: &str,
    ext: &str,
    format: OutputFormat,
) -> Result<()> {
    let cfg = Config::load()?.ipfs().cloned().unwrap_or_default();
    let filename = format!("mevlog-{hash}.{ext}");
    let result = ipfs::upload(&cfg, body.into_bytes(), &filename, content_type).await?;

    match format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            let payload = serde_json::json!({
                "cid": result.cid,
                "gateway_url": result.gateway_url,
                "pinata_gateway_url": result.pinata_gateway_url,
                "filename": filename,
            });
            let out = if matches!(format, OutputFormat::JsonPretty) {
                serde_json::to_string_pretty(&payload)?
            } else {
                serde_json::to_string(&payload)?
            };
            println!("{out}");
        }
        _ => {
            println!("Uploaded to IPFS");
            println!("  cid:     {}", result.cid);
            println!("  gateway: {}", result.gateway_url);
            if let Some(pinata_url) = &result.pinata_gateway_url {
                println!("  pinata:  {pinata_url}");
            }
            println!("  file:    {filename}");
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
