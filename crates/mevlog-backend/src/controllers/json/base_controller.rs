use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::misc::shared_init::mevlog_cmd_path;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::controllers::base_controller::{DATA_FETCH_ERROR, decorate_error_message};
use crate::misc::{prices::get_price_for_chain_id, rpc_utils::get_random_rpc_url};

/// Runs `block-txs` then `block-logs` for the same block and nests each tx's
/// logs under `result[].logs` grouped by `tx_index`.
pub(crate) async fn block_txs_with_logs(
    chain_id: u64,
    block_number: Option<String>,
) -> Result<Value, Value> {
    let block_arg = block_number.unwrap_or_else(|| "latest".to_string());
    let price = get_price_for_chain_id(chain_id).await.ok().flatten();
    let rpc_url = get_random_rpc_url(chain_id).await.ok().flatten();

    let mut txs_cmd = Command::new(mevlog_cmd_path());
    txs_cmd
        .arg("block-txs")
        .arg(&block_arg)
        .arg("--format")
        .arg("json")
        .arg("--rpc-timeout-ms")
        .arg("500")
        .arg("--latest-offset")
        .arg("2");
    txs_cmd.env("RUST_LOG", "off");
    if let Some(price) = price {
        txs_cmd.arg("--native-token-price").arg(price.to_string());
    }
    if let Some(rpc_url) = &rpc_url {
        txs_cmd.arg("--rpc-url").arg(rpc_url);
    }
    txs_cmd.arg("--chain-id").arg(chain_id.to_string());
    txs_cmd.arg("--skip-verify-chain-id");

    let mut txs_resp: Value = call_json_command_first_line(&mut txs_cmd).await?;

    let resolved_block = txs_resp
        .get("query")
        .and_then(|q| q.get("blocks"))
        .and_then(|b| b.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| block_arg.clone());

    let mut logs_cmd = Command::new(mevlog_cmd_path());
    logs_cmd
        .arg("block-logs")
        .arg(&resolved_block)
        .arg("--format")
        .arg("json")
        .arg("--rpc-timeout-ms")
        .arg("500");
    logs_cmd.env("RUST_LOG", "off");
    if let Some(rpc_url) = &rpc_url {
        logs_cmd.arg("--rpc-url").arg(rpc_url);
    }
    logs_cmd.arg("--chain-id").arg(chain_id.to_string());
    logs_cmd.arg("--skip-verify-chain-id");

    let logs_resp: Value = call_json_command_first_line(&mut logs_cmd).await?;

    let mut logs_by_tx: HashMap<i64, Vec<Value>> = HashMap::new();
    if let Some(rows) = logs_resp.get("result").and_then(|r| r.as_array()) {
        for log in rows {
            if let Some(tx_index) = log.get("tx_index").and_then(|v| v.as_i64()) {
                logs_by_tx.entry(tx_index).or_default().push(log.clone());
            }
        }
    }

    if let Some(txs) = txs_resp.get_mut("result").and_then(|r| r.as_array_mut()) {
        for tx in txs.iter_mut() {
            let tx_index = tx.get("tx_index").and_then(|v| v.as_i64());
            let logs = tx_index
                .and_then(|i| logs_by_tx.remove(&i))
                .unwrap_or_default();
            if let Some(obj) = tx.as_object_mut() {
                obj.insert("logs".to_string(), Value::Array(logs));
            }
        }
    }

    Ok(txs_resp)
}

#[hotpath::measure]
pub(crate) async fn call_json_command<T: serde::de::DeserializeOwned>(
    cmd: &mut Command,
) -> Result<T, Value> {
    let timeout_duration = Duration::from_secs(10);

    match timeout(timeout_duration, cmd.output()).await {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let error_msg = String::from_utf8_lossy(&output.stderr);

                let friendly_error = decorate_error_message(&error_msg);

                return Err(serde_json::json!({
                    "error": friendly_error
                }));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            match serde_json::from_str::<T>(&stdout) {
                Ok(value) => Ok(value),
                Err(e) => Err(serde_json::json!({
                    "error": format!("Failed to parse JSON: {e}")
                })),
            }
        }
        Ok(Err(e)) => Err(serde_json::json!({
            "error": e.to_string()
        })),
        Err(_) => Err(serde_json::json!({
            "error": decorate_error_message(DATA_FETCH_ERROR)
        })),
    }
}

pub(crate) async fn call_json_command_first_line<T: serde::de::DeserializeOwned>(
    cmd: &mut Command,
) -> Result<T, Value> {
    tracing::trace!("cmd: {:?}", &cmd);
    let timeout_duration = Duration::from_secs(10);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let timeout_result = timeout(timeout_duration, async {
        let mut child = cmd.spawn().map_err(|e| {
            serde_json::json!({
                "error": format!("Failed to spawn command: {e}")
            })
        })?;

        let stdout = child.stdout.take().ok_or_else(|| {
            serde_json::json!({
                "error": "Failed to capture stdout"
            })
        })?;

        let stderr = child.stderr.take();

        let mut reader = BufReader::new(stdout).lines();

        let next_line_future = hotpath::future!(reader.next_line(), log = true);

        if let Some(line) = next_line_future.await.map_err(|e| {
            serde_json::json!({
                "error": format!("Failed to read line: {e}")
            })
        })? {
            match serde_json::from_str::<T>(&line) {
                Ok(value) => Ok(value),
                Err(_) => Err(serde_json::json!({
                    "error": "No valid JSON found in output"
                })),
            }
        } else {
            let error_msg = if let Some(stderr) = stderr {
                let mut stderr_reader = BufReader::new(stderr).lines();
                let mut stderr_lines = Vec::new();
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    stderr_lines.push(line);
                }
                if stderr_lines.is_empty() {
                    "No output received from command".to_string()
                } else {
                    decorate_error_message(&stderr_lines.join("\n"))
                }
            } else {
                "No output received from command".to_string()
            };
            Err(serde_json::json!({
                "error": error_msg
            }))
        }
    })
    .await;

    match timeout_result {
        Ok(result) => result,
        Err(_) => Err(serde_json::json!({
            "error": decorate_error_message(DATA_FETCH_ERROR)
        })),
    }
}

pub(crate) fn extract_json_query_params<T>(
    query: Result<Query<T>, axum::extract::rejection::QueryRejection>,
) -> Result<T, impl IntoResponse>
where
    T: for<'de> Deserialize<'de>,
{
    match extract_query_params(query) {
        Ok(params) => Ok(params),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )),
    }
}

pub(crate) fn extract_query_params<T>(
    query: Result<Query<T>, axum::extract::rejection::QueryRejection>,
) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    match query {
        Ok(Query(params)) => Ok(params),
        Err(e) => Err(e.to_string()),
    }
}
