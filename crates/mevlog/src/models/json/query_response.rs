use comfy_table::Table;
use eyre::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{ChainInfoNoRpcsJson, misc::shared_init::TraceMode};

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

fn is_false(v: &bool) -> bool {
    !v
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryParams {
    pub blocks: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sql: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evm_trace: Option<TraceMode>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub evm_calls: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub evm_ops: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub evm_state_diff: bool,
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
    pub fn rows_as<T: DeserializeOwned>(&self) -> Result<Vec<T>> {
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
pub fn serialize_query_response(
    results: Vec<Value>,
    pretty: bool,
    chain: ChainInfoNoRpcsJson,
    duration_ns: u64,
    cached_blocks: u64,
    new_blocks: u64,
    query: QueryParams,
) -> serde_json::Result<String> {
    let envelope = QueryResponse {
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
}
