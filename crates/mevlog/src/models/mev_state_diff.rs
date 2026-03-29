use std::{collections::BTreeMap, fmt};

use alloy::primitives::{Address, B256, U256};
use colored::Colorize;

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

fn format_value(value: Option<B256>) -> String {
    match value {
        Some(v) => format!("{v}"),
        None => "null".to_string(),
    }
}

const COL_WIDTH: usize = 68;

impl fmt::Display for MEVStateDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{:<44} {:<COL_WIDTH$} {:<COL_WIDTH$} {:<COL_WIDTH$}",
            "ADDRESS", "SLOT", "BEFORE", "AFTER"
        )?;

        for (address, changes) in &self.contracts {
            let addr_str = format!("{address}");
            for (i, change) in changes.iter().enumerate() {
                let addr_display = if i == 0 {
                    addr_str.green().to_string()
                } else {
                    " ".repeat(42)
                };

                writeln!(
                    f,
                    "{:<44} {:<COL_WIDTH$} {:<COL_WIDTH$} {:<COL_WIDTH$}",
                    addr_display,
                    format!("{}", change.slot).yellow(),
                    format_value(change.value_before),
                    format_value(change.value_after),
                )?;
            }
        }

        Ok(())
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
