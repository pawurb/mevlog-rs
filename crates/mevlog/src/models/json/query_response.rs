use comfy_table::Table;
use eyre::Result;
use html_escape::{encode_double_quoted_attribute, encode_text};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{ChainInfoNoRpcsJson, misc::shared_init::TraceMode};

/// Maximum length (in characters) of the user-provided `--desc` query
/// description.
pub const MAX_QUERY_DESC_CHARS: usize = 960;

/// Renders a single cell value as a flat string: strings as-is (blob columns
/// are already 0x-hex), null as empty, everything else via its JSON form.
fn cell(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    }
}

fn row_record(columns: &[String], row: &Value) -> Vec<String> {
    let obj = row.as_object();
    columns
        .iter()
        .map(|col| cell(obj.and_then(|o| o.get(col))))
        .collect()
}

/// Serializes query result rows as CSV (header + one line per row).
pub fn rows_to_csv(columns: &[String], rows: &[Value]) -> Result<String> {
    let mut writer = csv::Writer::from_writer(vec![]);
    writer.write_record(columns)?;

    for row in rows {
        writer.write_record(row_record(columns, row))?;
    }

    let bytes = writer.into_inner().map_err(|e| eyre::eyre!(e))?;
    Ok(String::from_utf8(bytes)?)
}

/// Renders query result rows as a pretty ASCII table.
pub fn rows_to_table(columns: &[String], rows: &[Value]) -> String {
    let mut table = Table::new();
    table.set_header(columns);

    for row in rows {
        table.add_row(row_record(columns, row));
    }

    table.to_string()
}

/// Metadata rendered in the header block of the standalone HTML page.
pub struct HtmlMeta<'a> {
    pub chain_name: &'a str,
    pub chain_id: u64,
    pub blocks: Option<&'a str>,
    pub sql: Option<&'a str>,
    pub description: Option<&'a str>,
    pub row_count: usize,
    pub duration: &'a str,
}

/// A deterministic, collision-resistant hash of the query's rendered content
/// (chain + query + description + columns + rows, excluding the volatile
/// duration). Used to name the standalone HTML file so an identical result
/// always maps to the same filename. Truncated to 16 hex chars.
pub fn content_hash(
    chain: &ChainInfoNoRpcsJson,
    query: &QueryParams,
    description: Option<&str>,
    columns: &[String],
    rows: &[Value],
) -> String {
    #[derive(Serialize)]
    struct HashInput<'a> {
        chain: &'a ChainInfoNoRpcsJson,
        query: &'a QueryParams,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<&'a str>,
        columns: &'a [String],
        rows: &'a [Value],
    }

    let bytes = serde_json::to_vec(&HashInput {
        chain,
        query,
        description,
        columns,
        rows,
    })
    .expect("query content is always serializable");

    let digest = Sha256::digest(&bytes);
    hex::encode(&digest[..8])
}

/// Renders a single result cell as an escaped `<td>`, applying lightweight
/// value-aware styling (success pills, monospace/truncated hex blobs,
/// right-aligned numbers).
fn html_cell(column: &str, value: Option<&Value>) -> String {
    let raw = cell(value);

    if column == "success" {
        return match raw.as_str() {
            "1" | "true" => "<td><span class=\"pill ok\">1</span></td>".to_string(),
            "0" | "false" => "<td><span class=\"pill fail\">0</span></td>".to_string(),
            _ => format!("<td>{}</td>", encode_text(&raw)),
        };
    }

    let is_number = matches!(value, Some(Value::Number(_)));

    // Only middle-truncate ASCII hex blobs. `raw.is_ascii()` guarantees the
    // fixed byte offsets below land on character boundaries (never panics);
    // arbitrary multibyte text falls through to full, escaped rendering.
    if raw.starts_with("0x") && raw.len() > 20 && raw.is_ascii() {
        let head = &raw[..10];
        let tail = &raw[raw.len() - 6..];
        return format!(
            "<td class=\"mono\" title=\"{}\">{}…{}</td>",
            encode_double_quoted_attribute(&raw),
            encode_text(head),
            encode_text(tail),
        );
    }

    let class = if raw.starts_with("0x") {
        " class=\"mono\""
    } else if is_number {
        " class=\"num\""
    } else {
        ""
    };

    format!("<td{}>{}</td>", class, encode_text(&raw))
}

/// Renders query results as a self-contained HTML page: inline styling, a
/// metadata header, and a click-to-sort table. No external assets, so the file
/// works offline and when served from any host.
pub fn rows_to_html(columns: &[String], rows: &[Value], meta: &HtmlMeta) -> String {
    let mut header_cells = String::new();
    for col in columns {
        header_cells.push_str(&format!("<th>{}</th>", encode_text(col)));
    }

    let mut body = String::new();
    for row in rows {
        let obj = row.as_object();
        body.push_str("<tr>");
        for col in columns {
            body.push_str(&html_cell(col, obj.and_then(|o| o.get(col))));
        }
        body.push_str("</tr>");
    }

    let blocks = meta.blocks.map(encode_text).unwrap_or_default();
    let sql = meta.sql.map(encode_text).unwrap_or_default();
    let chain = encode_text(meta.chain_name);
    let title = encode_text(meta.description.unwrap_or("mevlog query results"));

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
:root {{ color-scheme: light dark; }}
* {{ box-sizing: border-box; }}
body {{ margin: 0; padding: 1.5rem; font: 14px/1.5 system-ui, -apple-system, sans-serif; background: #0f1116; color: #e6e6e6; }}
h1 {{ font-size: 1.1rem; margin: 0 0 1rem; }}
.meta {{ background: #171a21; border: 1px solid #262b36; border-radius: 8px; padding: 1rem; margin-bottom: 1rem; }}
.meta dl {{ display: grid; grid-template-columns: max-content 1fr; gap: .35rem 1rem; margin: 0; }}
.meta dt {{ color: #8b93a3; }}
.meta dd {{ margin: 0; }}
.meta code {{ display: block; white-space: pre-wrap; word-break: break-word; background: #0f1116; border: 1px solid #262b36; border-radius: 6px; padding: .5rem .6rem; font: 12px/1.4 ui-monospace, monospace; }}
.tablewrap {{ overflow-x: auto; border: 1px solid #262b36; border-radius: 8px; }}
table {{ border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums; }}
thead th {{ position: sticky; top: 0; background: #202634; color: #cfd6e4; text-align: left; padding: .5rem .7rem; cursor: pointer; user-select: none; white-space: nowrap; border-bottom: 1px solid #303748; }}
thead th:hover {{ background: #262d3d; }}
tbody td {{ padding: .4rem .7rem; border-bottom: 1px solid #1c212b; white-space: nowrap; }}
tbody tr:nth-child(even) {{ background: #141821; }}
tbody tr:hover {{ background: #1b2130; }}
td.mono {{ font-family: ui-monospace, monospace; }}
td.num {{ text-align: right; font-variant-numeric: tabular-nums; }}
.pill {{ display: inline-block; padding: .05rem .5rem; border-radius: 999px; font-size: 12px; font-weight: 600; }}
.pill.ok {{ background: #16351f; color: #4ade80; }}
.pill.fail {{ background: #3a1717; color: #f87171; }}
.empty {{ padding: 1rem; color: #8b93a3; }}
</style>
</head>
<body>
<h1>{title}</h1>
<div class="meta">
<dl>
<dt>chain</dt><dd>{chain} <span style="color:#8b93a3">(id {chain_id})</span></dd>
<dt>blocks</dt><dd>{blocks}</dd>
<dt>rows</dt><dd>{row_count}</dd>
<dt>duration</dt><dd>{duration}</dd>
<dt>sql</dt><dd><code>{sql}</code></dd>
</dl>
</div>
<div class="tablewrap">
<table id="results">
<thead><tr>{header_cells}</tr></thead>
<tbody>{body}</tbody>
</table>
</div>
{empty}
<script>
(function () {{
  var table = document.getElementById('results');
  if (!table) return;
  var headers = table.tHead.rows[0].cells;
  for (var i = 0; i < headers.length; i++) {{
    (function (col) {{
      headers[col].addEventListener('click', function () {{
        var tbody = table.tBodies[0];
        var rows = Array.prototype.slice.call(tbody.rows);
        var asc = headers[col].dataset.asc !== 'true';
        headers[col].dataset.asc = asc ? 'true' : 'false';
        var decimal = /^-?\d+(\.\d+)?$/;
        rows.sort(function (a, b) {{
          var x = a.cells[col].textContent.trim();
          var y = b.cells[col].textContent.trim();
          // Only compare numerically when both cells are plain decimals; hex
          // values like 0xaa parseFloat to 0 and must sort lexicographically.
          var num = decimal.test(x) && decimal.test(y);
          var cmp = num ? parseFloat(x) - parseFloat(y) : x.localeCompare(y);
          return asc ? cmp : -cmp;
        }});
        rows.forEach(function (r) {{ tbody.appendChild(r); }});
      }});
    }})(i);
  }}
}})();
</script>
</body>
</html>
"#,
        title = title,
        chain = chain,
        chain_id = meta.chain_id,
        blocks = blocks,
        row_count = meta.row_count,
        duration = encode_text(meta.duration),
        sql = sql,
        header_cells = header_cells,
        body = body,
        empty = if rows.is_empty() {
            "<div class=\"empty\">No rows.</div>"
        } else {
            ""
        },
    )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm_trace: Option<TraceMode>,
}

/// In-process result of a SQL-backed command, produced by the `cmds` layer.
///
/// Carries the generic `columns + rows` from `run_raw_query` (so the CLI can
/// render the response envelope, CSV, or table without any change to ordering)
/// plus the metadata the JSON envelope needs. In-process consumers (e.g. the
/// TUI) deserialize the rows into concrete types via [`QueryOutcome::rows_as`].
pub struct QueryOutcome {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
    pub cached_blocks: u64,
    pub new_blocks: u64,
    pub duration_ns: u64,
    pub chain: ChainInfoNoRpcsJson,
    pub query: QueryParams,
}

impl QueryOutcome {
    /// Deserializes the result rows into a concrete type. Used by the typed
    /// `cmds` wrappers so callers never touch raw JSON.
    pub(crate) fn rows_as<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
        self.rows
            .iter()
            .map(|row| serde_json::from_value(row.clone()).map_err(Into::into))
            .collect()
    }
}

pub fn format_duration(ns: u64) -> String {
    if ns < 1_000 {
        format!("{} ns", ns)
    } else if ns < 1_000_000 {
        format!("{:.2} µs", ns as f64 / 1_000.0)
    } else if ns < 1_000_000_000 {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    } else {
        format!("{:.2} s", ns as f64 / 1_000_000_000.0)
    }
}

/// Standard response envelope emitted by the SQLite-backed query path.
#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub result: Vec<Value>,
    pub result_count: usize,
    pub cached_blocks: u64,
    pub new_blocks: u64,
    pub duration: String,
    pub chain: ChainInfoNoRpcsJson,
    pub query: QueryParams,
}

/// Serializes SQL result rows into the standard response envelope used by the
/// SQLite-backed query path.
#[allow(clippy::too_many_arguments)]
pub fn serialize_query_response(
    results: Vec<Value>,
    pretty: bool,
    chain: ChainInfoNoRpcsJson,
    duration_ns: u64,
    cached_blocks: u64,
    new_blocks: u64,
    query: QueryParams,
    description: Option<String>,
) -> serde_json::Result<String> {
    let envelope = QueryResponse {
        description,
        result_count: results.len(),
        result: results,
        cached_blocks,
        new_blocks,
        duration: format_duration(duration_ns),
        chain,
        query,
    };

    if pretty {
        serde_json::to_string_pretty(&envelope)
    } else {
        serde_json::to_string(&envelope)
    }
}

#[cfg(test)]
mod test {
    use serde_json::json;

    use super::*;

    fn sample_columns() -> Vec<String> {
        [
            "block_number",
            "tx_hash",
            "signature",
            "success",
            "to_address",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    fn sample_rows() -> Vec<Value> {
        vec![
            json!({
                "block_number": 100,
                "tx_hash": "0xaa",
                "signature": "transfer(address,uint256)",
                "success": true,
                "to_address": Value::Null,
            }),
            json!({
                "block_number": 101,
                "tx_hash": "0xbb",
                "signature": "swap, exact",
                "success": false,
                "to_address": "0x22",
            }),
        ]
    }

    #[test]
    fn csv_writes_header_and_rows() {
        let csv = rows_to_csv(&sample_columns(), &sample_rows()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();

        assert_eq!(
            lines[0],
            "block_number,tx_hash,signature,success,to_address"
        );
        // Null renders empty; the signature contains a comma so the csv writer
        // quotes it.
        assert_eq!(lines[1], "100,0xaa,\"transfer(address,uint256)\",true,");
        assert_eq!(lines[2], "101,0xbb,\"swap, exact\",false,0x22");
    }

    #[test]
    fn csv_with_no_rows_still_writes_the_header() {
        let csv = rows_to_csv(&sample_columns(), &[]).unwrap();
        assert_eq!(csv, "block_number,tx_hash,signature,success,to_address\n");
    }

    #[test]
    fn table_contains_headers_and_values() {
        let table = rows_to_table(&sample_columns(), &sample_rows());
        assert!(table.contains("block_number"));
        assert!(table.contains("transfer(address,uint256)"));
        assert!(table.contains("0xbb"));
    }

    #[test]
    fn table_with_no_rows_still_contains_headers() {
        let table = rows_to_table(&sample_columns(), &[]);
        assert!(table.contains("block_number"));
        assert!(table.contains("to_address"));
    }

    fn sample_chain() -> ChainInfoNoRpcsJson {
        ChainInfoNoRpcsJson {
            chain_id: 1,
            name: "Ethereum".to_string(),
            currency: "ETH".to_string(),
            explorer_url: None,
            native_token_price: None,
        }
    }

    fn sample_query() -> QueryParams {
        QueryParams {
            blocks: Some("100:101".to_string()),
            sql: Some("SELECT * FROM transactions".to_string()),
            evm_trace: None,
        }
    }

    fn sample_meta() -> HtmlMeta<'static> {
        HtmlMeta {
            chain_name: "Ethereum",
            chain_id: 1,
            blocks: Some("100:101"),
            sql: Some("SELECT * FROM transactions"),
            description: None,
            row_count: 2,
            duration: "1.23 ms",
        }
    }

    #[test]
    fn html_contains_headers_values_and_meta() {
        let html = rows_to_html(&sample_columns(), &sample_rows(), &sample_meta());
        assert!(html.contains("<th>block_number</th>"));
        assert!(html.contains("transfer(address,uint256)"));
        assert!(html.contains("0xbb"));
        assert!(html.contains("Ethereum"));
        assert!(html.contains("SELECT * FROM transactions"));
        assert!(html.contains("pill ok"));
        assert!(html.contains("pill fail"));
    }

    #[test]
    fn html_escapes_cell_contents() {
        let columns = vec!["payload".to_string()];
        let rows = vec![json!({ "payload": "<script>alert(1)</script>" })];
        let html = rows_to_html(&columns, &rows, &sample_meta());
        assert!(!html.contains("<script>alert(1)</script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn html_does_not_panic_on_multibyte_0x_prefixed_text() {
        // A non-ASCII value starting with "0x" and longer than 20 bytes must not
        // be byte-sliced at fixed offsets (would panic on a char boundary).
        let columns = vec!["note".to_string()];
        let long = format!("0x{}", "é".repeat(20));
        let rows = vec![json!({ "note": long })];
        let html = rows_to_html(&columns, &rows, &sample_meta());
        assert!(html.contains(&"é".repeat(20)));
    }

    #[test]
    fn html_with_no_rows_still_renders_header_and_meta() {
        let html = rows_to_html(&sample_columns(), &[], &sample_meta());
        assert!(html.contains("<th>block_number</th>"));
        assert!(html.contains("Ethereum"));
        assert!(html.contains("No rows."));
    }

    #[test]
    fn content_hash_is_stable_and_sensitive() {
        let cols = sample_columns();
        let rows = sample_rows();
        let h1 = content_hash(&sample_chain(), &sample_query(), None, &cols, &rows);
        let h2 = content_hash(&sample_chain(), &sample_query(), None, &cols, &rows);
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);

        let mut changed = rows.clone();
        changed.push(json!({ "block_number": 999 }));
        let h3 = content_hash(&sample_chain(), &sample_query(), None, &cols, &changed);
        assert_ne!(h1, h3);

        let h4 = content_hash(
            &sample_chain(),
            &sample_query(),
            Some("weekly USDC report"),
            &cols,
            &rows,
        );
        assert_ne!(h1, h4);
    }

    #[test]
    fn html_title_uses_description_when_present() {
        let meta = HtmlMeta {
            description: Some("Top gas burners <script>"),
            ..sample_meta()
        };
        let html = rows_to_html(&sample_columns(), &sample_rows(), &meta);
        assert!(html.contains("<title>Top gas burners &lt;script&gt;</title>"));
        assert!(html.contains("<h1>Top gas burners &lt;script&gt;</h1>"));
        assert!(!html.contains("mevlog query results"));

        let html = rows_to_html(&sample_columns(), &sample_rows(), &sample_meta());
        assert!(html.contains("<title>mevlog query results</title>"));
        assert!(html.contains("<h1>mevlog query results</h1>"));
    }

    #[test]
    fn envelope_includes_description_only_when_present() {
        let body = serialize_query_response(
            sample_rows(),
            false,
            sample_chain(),
            1_000,
            0,
            0,
            sample_query(),
            Some("weekly USDC report".to_string()),
        )
        .unwrap();
        let parsed: QueryResponse = serde_json::from_str(&body).unwrap();
        assert_eq!(parsed.description.as_deref(), Some("weekly USDC report"));

        let body = serialize_query_response(
            sample_rows(),
            false,
            sample_chain(),
            1_000,
            0,
            0,
            sample_query(),
            None,
        )
        .unwrap();
        assert!(!body.contains("description"));
    }
}
