use revm::primitives::Address;

#[derive(Debug, Clone, PartialEq)]
pub struct MEVAddress {
    address: Address,
}

impl MEVAddress {
    pub fn new(address: Address) -> Self {
        Self { address }
    }

    pub fn address(&self) -> Address {
        self.address
    }
}
