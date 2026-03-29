use revm::primitives::Address;
use serde::{Deserialize, Serialize};

use crate::models::{json::mev_log_json::MEVLogJson, mev_log_group::MEVLogGroup};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MEVLogGroupJson {
    pub source: Address,
    pub logs: Vec<MEVLogJson>,
}

impl From<&MEVLogGroup> for MEVLogGroupJson {
    fn from(log_group: &MEVLogGroup) -> Self {
        let logs = log_group.logs.iter().map(MEVLogJson::from).collect();

        Self {
            source: log_group.source(),
            logs,
        }
    }
}
