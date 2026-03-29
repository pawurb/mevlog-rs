use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

use crate::models::mev_log::MEVLog;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MEVLogJson {
    pub source: Address,
    pub signature: String,
    pub symbol: Option<String>,
    pub amount: Option<String>,
    pub topics: Vec<FixedBytes<32>>,
    pub data: String,
}

impl From<&MEVLog> for MEVLogJson {
    fn from(log: &MEVLog) -> Self {
        Self {
            source: log.source,
            signature: log.signature.signature.clone(),
            symbol: log.signature.symbol.clone(),
            amount: log.signature.amount.map(|amt| amt.to_string()),
            topics: log.topics.clone(),
            data: hex::encode(log.data.clone()),
        }
    }
}
