use std::fmt;

use colored::Colorize;
use revm::primitives::Address;

use super::mev_log::MEVLog;
use crate::misc::shared_init::EVMChain;

#[derive(Debug)]
pub struct MEVLogGroup {
    source: Address,
    pub logs: Vec<MEVLog>,
    pub chain: EVMChain,
}

impl MEVLogGroup {
    pub fn new(source: Address, logs: Vec<MEVLog>, chain: EVMChain) -> Self {
        Self {
            source,
            logs,
            chain,
        }
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
            format!("{}/address/{}", self.chain.etherscan_url(), self.source).green()
        )?;
        for log in &self.logs {
            writeln!(f, "    {log}")?;
        }
        Ok(())
    }
}
