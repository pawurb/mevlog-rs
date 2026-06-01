use revm::primitives::Address;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExtract {
    pub from: Address,
    pub to: Address,
    pub signature: String,
    pub signature_hash: Option<String>,
}
