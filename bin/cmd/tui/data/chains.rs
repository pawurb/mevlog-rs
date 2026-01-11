use eyre::Result;
use mevlog::ChainEntryJson;
use std::{process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::timeout,
};

use crate::cmd::tui::data::mevlog_cmd;

pub async fn fetch_chains(filter: Option<String>) -> Result<Vec<ChainEntryJson>> {
    let mut cmd = mevlog_cmd();
    cmd.arg("chains").arg("--format").arg("json");

    if let Some(filter) = filter {
        cmd.arg("--filter").arg(filter);
    }

    cmd.env("RUST_LOG", "off")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let timeout_duration = Duration::from_secs(10);

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
            if let Ok(chains) = serde_json::from_str::<Vec<ChainEntryJson>>(&line) {
                return Ok(chains);
            }
            return Err(eyre::eyre!("Failed to parse chains response: {}", line));
        }

        if let Some(line) = stderr_reader.next_line().await? {
            return Err(eyre::eyre!("{}", line));
        }

        Ok::<_, eyre::Error>(vec![])
    })
    .await;

    match result {
        Ok(chains) => chains,
        Err(_) => eyre::bail!("mevlog chains timed out after 10 seconds"),
    }
}
