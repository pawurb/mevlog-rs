use std::{collections::HashMap, fs, path::PathBuf};

use eyre::Result;
use serde::{Deserialize, Serialize};

use crate::misc::shared_init::config_path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub chains: HashMap<u64, ChainConfig>,
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
            fs::write(&path, "")?;
        }
        Ok(())
    }

    pub fn get_chain(&self, chain_id: u64) -> Option<&ChainConfig> {
        self.chains.get(&chain_id)
    }
}
