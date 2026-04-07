use std::sync::Arc;

use revm::primitives::Address;

use super::mev_log::MEVLog;
use crate::models::evm_chain::EVMChain;

#[derive(Debug)]
pub struct MEVLogGroup {
    source: Address,
    pub logs: Vec<MEVLog>,
    pub chain: Arc<EVMChain>,
}

#[hotpath::measure_all]
impl MEVLogGroup {
    pub fn new(source: Address, logs: Vec<MEVLog>, chain: Arc<EVMChain>) -> Self {
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
