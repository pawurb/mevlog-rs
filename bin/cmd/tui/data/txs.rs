use std::{process::Stdio, sync::Arc, time::Duration};

use eyre::Result;
use mevlog::misc::rpc_capability::is_debug_trace_available;
use mevlog::misc::shared_init::{TraceMode, init_provider};
use mevlog::models::json::mev_opcode_json::MEVOpcodeJson;
use mevlog::models::mev_transaction::CallExtract;
use serde::Deserialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::timeout,
};

use crate::cmd::tui::data::{MEVTransactionJson, SearchFilters, mevlog_cmd};

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct TxWithOpcodes {
    opcodes: Option<Vec<MEVOpcodeJson>>,
}

#[derive(Deserialize)]
struct TxWithCalls {
    calls: Option<Vec<CallExtract>>,
}

pub async fn fetch_txs(
    filters: &SearchFilters,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
) -> Result<Vec<MEVTransactionJson>> {
    let mut cmd = mevlog_cmd();

    cmd.arg("search")
        .arg("-b")
        .arg(&filters.blocks)
        .arg("--format")
        .arg("json");

    if let Some(ref pos) = filters.position {
        cmd.arg("--position").arg(pos);
    }
    if let Some(ref from) = filters.from {
        cmd.arg("--from").arg(from);
    }
    if let Some(ref to) = filters.to {
        cmd.arg("--to").arg(to);
    }
    if let Some(ref event) = filters.event {
        cmd.arg("--event").arg(event);
    }
    if let Some(ref not_event) = filters.not_event {
        cmd.arg("--not-event").arg(not_event);
    }
    if let Some(ref method) = filters.method {
        cmd.arg("--method").arg(method);
    }
    if let Some(ref erc20) = filters.erc20_transfer {
        cmd.arg("--erc20-transfer").arg(erc20);
    }
    if let Some(ref tx_cost) = filters.tx_cost {
        cmd.arg("--tx-cost").arg(tx_cost);
    }
    if let Some(ref gas_price) = filters.gas_price {
        cmd.arg("--gas-price").arg(gas_price);
    }

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
            if let Ok(txs) = serde_json::from_str::<Vec<MEVTransactionJson>>(&line) {
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
        Err(_) => eyre::bail!("mevlog search timed out after 120 seconds"),
    }
}

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

pub async fn fetch_opcodes(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
    trace_mode: TraceMode,
) -> Result<Vec<MEVOpcodeJson>> {
    let mut cmd = mevlog_cmd();

    cmd.arg("tx")
        .arg(tx_hash)
        .arg("--trace")
        .arg(trace_mode.to_string())
        .arg("--ops")
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
            if let Ok(txs) = serde_json::from_str::<Vec<TxWithOpcodes>>(&line) {
                let opcodes = txs
                    .into_iter()
                    .next()
                    .and_then(|tx| tx.opcodes)
                    .unwrap_or_default();
                return Ok(opcodes);
            }

            return Err(eyre::eyre!("Failed to parse opcodes response: {}", line));
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
        Ok(opcodes) => opcodes,
        Err(_) => eyre::bail!("mevlog tx --ops timed out after 120 seconds"),
    }
}

pub async fn fetch_traces(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
    trace_mode: TraceMode,
) -> Result<Vec<CallExtract>> {
    let mut cmd = mevlog_cmd();

    cmd.arg("tx")
        .arg(tx_hash)
        .arg("--trace")
        .arg(trace_mode.to_string())
        .arg("--show-calls")
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
            if let Ok(txs) = serde_json::from_str::<Vec<TxWithCalls>>(&line) {
                let calls = txs
                    .into_iter()
                    .next()
                    .and_then(|tx| tx.calls)
                    .unwrap_or_default();
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
        Err(_) => eyre::bail!("mevlog tx --show-calls timed out after 120 seconds"),
    }
}

pub async fn fetch_tx_with_trace(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
    trace_mode: TraceMode,
) -> Result<MEVTransactionJson> {
    let mut cmd = mevlog_cmd();

    cmd.arg("tx")
        .arg(tx_hash)
        .arg("--trace")
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
            if let Ok(txs) = serde_json::from_str::<Vec<MEVTransactionJson>>(&line) {
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
        Err(_) => eyre::bail!("mevlog tx --trace timed out after 120 seconds"),
    }
}
