use revm::primitives::U256;

use crate::misc::utils::UNKNOWN;

#[derive(Debug, Clone)]
pub struct MEVLogSignature {
    pub signature: String,
    pub amount: Option<U256>,
}

impl MEVLogSignature {
    pub fn new(signature_str: Option<String>) -> Self {
        let signature_str = signature_str.unwrap_or(UNKNOWN.to_string());

        Self {
            signature: signature_str,
            amount: None,
        }
    }

    pub fn with_amount(mut self, amount: Option<U256>) -> Self {
        self.amount = amount;
        self
    }
}
