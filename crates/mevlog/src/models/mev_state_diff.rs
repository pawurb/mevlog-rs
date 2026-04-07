use std::collections::BTreeMap;

use alloy::primitives::{Address, B256, U256};

#[derive(Clone, Debug)]
pub struct StorageSlotChange {
    pub slot: B256,
    pub value_before: Option<B256>,
    pub value_after: Option<B256>,
}

impl StorageSlotChange {
    pub fn new(slot: B256, value_before: Option<B256>, value_after: Option<B256>) -> Self {
        Self {
            slot,
            value_before,
            value_after,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct MEVStateDiff {
    pub contracts: BTreeMap<Address, Vec<StorageSlotChange>>,
}

impl MEVStateDiff {
    pub fn new() -> Self {
        Self {
            contracts: BTreeMap::new(),
        }
    }

    pub fn add_change(
        &mut self,
        address: Address,
        slot: B256,
        value_before: Option<B256>,
        value_after: Option<B256>,
    ) {
        let changes = self.contracts.entry(address).or_default();
        changes.push(StorageSlotChange::new(slot, value_before, value_after));
    }

    pub fn is_empty(&self) -> bool {
        self.contracts.is_empty()
    }
}

pub fn b256_from_u256(value: U256) -> B256 {
    B256::from(value.to_be_bytes())
}

pub fn u256_to_option_b256(value: U256) -> Option<B256> {
    if value == U256::ZERO {
        None
    } else {
        Some(b256_from_u256(value))
    }
}
