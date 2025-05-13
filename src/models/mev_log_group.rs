use std::fmt;

use colored::Colorize;
use revm::primitives::Address;

use super::mev_log::MEVLog;
use crate::misc::utils::ETHERSCAN_URL;

#[derive(Debug)]
pub struct MEVLogGroup {
    source: Address,
    pub logs: Vec<MEVLog>,
}

impl MEVLogGroup {
    pub fn new(source: Address, logs: Vec<MEVLog>) -> Self {
        Self { source, logs }
    }

    pub fn source(&self) -> Address {
        self.source
    }

    pub fn add_log(&mut self, log: MEVLog) {
        self.logs.push(log);
    }
}

impl fmt::Display for MEVLogGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "  {}",
            format!("{}/address/{}", ETHERSCAN_URL, self.source).green()
        )?;
        for log in &self.logs {
            writeln!(f, "    {log}")?;
        }
        Ok(())
    }
}
