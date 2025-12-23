use eyre::Result;
use tokio::process::Command;

use crate::cmd::tui::data::TxRow;

#[derive(Clone)]
pub struct DataFetcher {
    pub rpc_url: Option<String>,
    pub chain_id: Option<u64>,
}

impl DataFetcher {
    pub fn new(rpc_url: Option<String>, chain_id: Option<u64>) -> Self {
        Self { rpc_url, chain_id }
    }

    /// Fetch transactions for given blocks using mevlog CLI (async)
    pub async fn fetch(&self, blocks: &str) -> Result<Vec<TxRow>> {
        let mut cmd = Command::new("mevlog");

        cmd.arg("search")
            .arg("-b")
            .arg(blocks)
            .arg("--format")
            .arg("json");

        if let Some(rpc_url) = &self.rpc_url {
            cmd.arg("--rpc-url").arg(rpc_url);
        } else if let Some(chain_id) = self.chain_id {
            cmd.arg("--chain-id").arg(chain_id.to_string());
        }

        cmd.env("RUST_LOG", "off");

        let output = cmd.output().await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eyre::bail!("mevlog search failed: {}", stderr);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let txs: Vec<TxRow> = serde_json::from_str(&stdout)?;

        Ok(txs)
    }
}
