use std::{process::Stdio, time::Duration};

use eyre::Result;
use mevlog::models::json::mev_opcode_json::MEVOpcodeJson;
use serde::Deserialize;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::timeout,
};

use crate::cmd::tui::data::MEVTransactionJson;

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct TxWithOpcodes {
    opcodes: Option<Vec<MEVOpcodeJson>>,
}

pub async fn fetch_txs(
    blocks: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
) -> Result<Vec<MEVTransactionJson>> {
    let mut cmd = Command::new("mevlog");

    cmd.arg("search")
        .arg("-b")
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

    let timeout_duration = Duration::from_secs(60);

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
        Err(_) => eyre::bail!("mevlog search timed out after 60 seconds"),
    }
}

pub async fn fetch_opcodes(
    tx_hash: &str,
    rpc_url: Option<String>,
    chain_id: Option<u64>,
) -> Result<Vec<MEVOpcodeJson>> {
    let mut cmd = Command::new("mevlog");

    cmd.arg("tx")
        .arg(tx_hash)
        .arg("--trace")
        .arg("revm")
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
