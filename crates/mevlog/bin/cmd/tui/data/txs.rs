use std::{process::Stdio, sync::Arc, time::Duration};

use eyre::Result;
use mevlog::misc::rpc_capability::is_debug_trace_available;
use mevlog::misc::shared_init::{TraceMode, init_provider};
use mevlog::models::call_extract::CallExtract;
use mevlog::models::json::state_diff_json::StateDiffJson;
use serde::Deserialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::timeout,
};
use tracing::debug;

use crate::cmd::tui::data::{LogJson, TransactionJson, mevlog_cmd};

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct Envelope<T> {
    result: Vec<T>,
}

#[hotpath::measure(future = true)]
pub async fn fetch_txs(
    blocks: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
) -> Result<Vec<TransactionJson>> {
    let mut cmd = mevlog_cmd();

    cmd.arg("block-txs").arg(blocks).arg("--format").arg("json");

    if let Some(rpc_url) = &rpc_url {
        cmd.arg("--rpc-url").arg(rpc_url);
    } else if let Some(chain_id) = chain_id {
        cmd.arg("--chain-id").arg(chain_id.to_string());
    }

    let cmd_args: Vec<_> = cmd
        .as_std()
        .get_args()
        .map(|a| a.to_string_lossy())
        .collect();
    debug!(cmd = %cmd_args.join(" "), "mevlog command");

    cmd.env("RUST_LOG", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timeout_duration = Duration::from_secs(120);

    let result = timeout(timeout_duration, async {
        let mut child = cmd.spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        if let Some(line) = stdout_reader.next_line().await? {
            if let Ok(envelope) = serde_json::from_str::<Envelope<TransactionJson>>(&line) {
                let txs = envelope.result;
                return Ok(txs);
            }

            return Err(eyre::eyre!("Failed to parse response: {}", line));
        }

        if let Some(line) = stderr_reader.next_line().await? {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&line) {
                return Err(eyre::eyre!("{}", error_response.error));
            }

            return Err(eyre::eyre!("{}", line));
        }

        Ok::<_, eyre::Error>(vec![])
    })
    .await;

    match result {
        Ok(txs) => txs,
        Err(_) => eyre::bail!("mevlog block-txs timed out after 120 seconds"),
    }
}

/// Fetch the txs of a block together with all its logs, attaching each tx's logs
/// in memory. The block is indexed once by `block-txs`, so the follow-up
/// `block-logs` call is a local read (no extra RPC). Logs are grouped by
/// `tx_index`, which avoids the per-tx `tx-logs` RPC the popup used to make on
/// every selection change.
#[hotpath::measure(future = true)]
pub async fn fetch_txs_with_logs(
    blocks: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
) -> Result<Vec<TransactionJson>> {
    let mut txs = fetch_txs(blocks, rpc_url.clone(), chain_id).await?;
    if txs.is_empty() {
        return Ok(txs);
    }

    // Pin the logs query to the concrete block number we just fetched so a "latest"
    // request can't race a newly produced block between the two subprocess calls.
    let block_number = txs[0].block_number;
    let logs = fetch_block_logs(&block_number.to_string(), rpc_url, chain_id).await?;

    let mut by_tx_index: std::collections::HashMap<u64, Vec<LogJson>> =
        std::collections::HashMap::new();
    for log in logs {
        if let Some(tx_index) = log.tx_index {
            by_tx_index.entry(tx_index).or_default().push(log);
        }
    }
    for tx in &mut txs {
        if let Some(logs) = by_tx_index.remove(&tx.tx_index) {
            tx.logs = logs;
        }
    }

    Ok(txs)
}

#[hotpath::measure(future = true)]
async fn fetch_block_logs(
    blocks: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
) -> Result<Vec<LogJson>> {
    let mut cmd = mevlog_cmd();

    cmd.arg("block-logs")
        .arg(blocks)
        .arg("--format")
        .arg("json");

    if let Some(rpc_url) = &rpc_url {
        cmd.arg("--rpc-url").arg(rpc_url);
    } else if let Some(chain_id) = chain_id {
        cmd.arg("--chain-id").arg(chain_id.to_string());
    }

    cmd.env("RUST_LOG", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timeout_duration = Duration::from_secs(120);

    let result = timeout(timeout_duration, async {
        let mut child = cmd.spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        if let Some(line) = stdout_reader.next_line().await? {
            if let Ok(envelope) = serde_json::from_str::<Envelope<LogJson>>(&line) {
                return Ok(envelope.result);
            }

            return Err(eyre::eyre!("Failed to parse logs response: {}", line));
        }

        if let Some(line) = stderr_reader.next_line().await? {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&line) {
                return Err(eyre::eyre!("{}", error_response.error));
            }

            return Err(eyre::eyre!("{}", line));
        }

        Ok::<_, eyre::Error>(vec![])
    })
    .await;

    match result {
        Ok(logs) => logs,
        Err(_) => eyre::bail!("mevlog block-logs timed out after 120 seconds"),
    }
}

#[hotpath::measure(log = true, future = true)]
pub async fn detect_trace_mode(rpc_url: &str) -> TraceMode {
    let Ok(provider) = init_provider(rpc_url).await else {
        return TraceMode::Revm;
    };
    let provider = Arc::new(provider);
    if is_debug_trace_available(&provider, 5000).await {
        TraceMode::RPC
    } else {
        TraceMode::Revm
    }
}

#[hotpath::measure(log = true, future = true)]
pub async fn fetch_traces(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
    trace_mode: TraceMode,
) -> Result<Vec<CallExtract>> {
    let mut cmd = mevlog_cmd();

    cmd.arg("evm-traces")
        .arg(tx_hash)
        .arg("--evm-trace")
        .arg(trace_mode.to_string())
        .arg("--format")
        .arg("json");

    if let Some(rpc_url) = &rpc_url {
        cmd.arg("--rpc-url").arg(rpc_url);
    } else if let Some(chain_id) = chain_id {
        cmd.arg("--chain-id").arg(chain_id.to_string());
    }

    cmd.env("RUST_LOG", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timeout_duration = Duration::from_secs(120);

    let result = timeout(timeout_duration, async {
        let mut child = cmd.spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        if let Some(line) = stdout_reader.next_line().await? {
            if let Ok(calls) = serde_json::from_str::<Vec<CallExtract>>(&line) {
                return Ok(calls);
            }

            return Err(eyre::eyre!("Failed to parse traces response: {}", line));
        }

        if let Some(line) = stderr_reader.next_line().await? {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&line) {
                return Err(eyre::eyre!("{}", error_response.error));
            }

            return Err(eyre::eyre!("{}", line));
        }

        Ok::<_, eyre::Error>(vec![])
    })
    .await;

    match result {
        Ok(traces) => traces,
        Err(_) => eyre::bail!("mevlog evm-traces timed out after 120 seconds"),
    }
}

#[hotpath::measure(future = true)]
pub async fn fetch_tx_with_trace(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
    trace_mode: TraceMode,
) -> Result<TransactionJson> {
    let mut cmd = mevlog_cmd();

    cmd.arg("tx")
        .arg(tx_hash)
        .arg("--evm-trace")
        .arg(trace_mode.to_string())
        .arg("--format")
        .arg("json");

    if let Some(rpc_url) = &rpc_url {
        cmd.arg("--rpc-url").arg(rpc_url);
    } else if let Some(chain_id) = chain_id {
        cmd.arg("--chain-id").arg(chain_id.to_string());
    }

    cmd.env("RUST_LOG", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timeout_duration = Duration::from_secs(120);

    let result = timeout(timeout_duration, async {
        let mut child = cmd.spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        if let Some(line) = stdout_reader.next_line().await? {
            if let Ok(envelope) = serde_json::from_str::<Envelope<TransactionJson>>(&line) {
                let txs = envelope.result;
                if let Some(tx) = txs.into_iter().next() {
                    return Ok(tx);
                }
                return Err(eyre::eyre!("No transaction in response"));
            }

            return Err(eyre::eyre!("Failed to parse tx trace response: {}", line));
        }

        if let Some(line) = stderr_reader.next_line().await? {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&line) {
                return Err(eyre::eyre!("{}", error_response.error));
            }

            return Err(eyre::eyre!("{}", line));
        }

        Err(eyre::eyre!("No output from mevlog tx"))
    })
    .await;

    match result {
        Ok(tx) => tx,
        Err(_) => eyre::bail!("mevlog tx --evm-trace timed out after 120 seconds"),
    }
}

#[hotpath::measure(future = true)]
pub async fn fetch_state_diff(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
    trace_mode: TraceMode,
) -> Result<StateDiffJson> {
    let mut cmd = mevlog_cmd();

    cmd.arg("evm-state-diff")
        .arg(tx_hash)
        .arg("--evm-trace")
        .arg(trace_mode.to_string())
        .arg("--format")
        .arg("json");

    if let Some(rpc_url) = &rpc_url {
        cmd.arg("--rpc-url").arg(rpc_url);
    } else if let Some(chain_id) = chain_id {
        cmd.arg("--chain-id").arg(chain_id.to_string());
    }

    cmd.env("RUST_LOG", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timeout_duration = Duration::from_secs(120);

    let result = timeout(timeout_duration, async {
        let mut child = cmd.spawn()?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| eyre::eyre!("Failed to capture stderr"))?;

        let mut stdout_reader = BufReader::new(stdout).lines();
        let mut stderr_reader = BufReader::new(stderr).lines();

        if let Some(line) = stdout_reader.next_line().await? {
            if let Ok(state_diff) = serde_json::from_str::<StateDiffJson>(&line) {
                return Ok(state_diff);
            }

            return Err(eyre::eyre!("Failed to parse state diff response: {}", line));
        }

        if let Some(line) = stderr_reader.next_line().await? {
            if let Ok(error_response) = serde_json::from_str::<ErrorResponse>(&line) {
                return Err(eyre::eyre!("{}", error_response.error));
            }

            return Err(eyre::eyre!("{}", line));
        }

        Ok::<_, eyre::Error>(StateDiffJson::default())
    })
    .await;

    match result {
        Ok(state_diff) => state_diff,
        Err(_) => eyre::bail!("mevlog evm-state-diff timed out after 120 seconds"),
    }
}
