use std::{collections::HashMap, fs, path::PathBuf};

use eyre::Result;
use serde::{Deserialize, Serialize};

use crate::misc::shared_init::config_path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    chains: HashMap<String, ChainConfig>,
}

impl Config {
    pub fn chains(&self) -> impl Iterator<Item = (u64, &ChainConfig)> {
        self.chains
            .iter()
            .filter_map(|(k, v)| k.parse::<u64>().ok().map(|id| (id, v)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub rpc_url: String,
}

impl Config {
    pub fn config_file_path() -> PathBuf {
        config_path().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_file_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn init_if_missing() -> Result<()> {
        let path = Self::config_file_path();
        if !path.exists() {
            fs::create_dir_all(config_path())?;
            fs::write(&path, Self::default_config_template())?;
        }
        Ok(())
    }

    fn default_config_template() -> &'static str {
        r#"# mevlog configuration file
#
# Configure custom RPC endpoints for each chain by chain ID.
# Uncomment and modify the examples below as needed.
#
# [chains.1]
# rpc_url = "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
#
# [chains.42161]
# rpc_url = "https://arb-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
"#
    }

    pub fn get_chain(&self, chain_id: u64) -> Option<&ChainConfig> {
        let key = chain_id.to_string();
        self.chains.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unquoted() {
        let content = r#"
[chains.1]
rpc_url = "https://example.com"
"#;
        let config: Config = toml::from_str(content).unwrap();
        assert!(config.get_chain(1).is_some());
    }

    #[test]
    fn test_parse_quoted() {
        let content = r#"
[chains."1"]
rpc_url = "https://example.com"
"#;
        let config: Config = toml::from_str(content).unwrap();
        assert!(config.get_chain(1).is_some());
    }
}
