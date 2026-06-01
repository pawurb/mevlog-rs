use revm::primitives::Address;
use serde::{Deserialize, Serialize};

use crate::models::json::mev_log_json::MEVLogJson;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MEVLogGroupJson {
    pub source: Address,
    pub logs: Vec<MEVLogJson>,
}
