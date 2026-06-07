use std::collections::BTreeMap;

use alloy::primitives::{Address, B256};
use serde::{Deserialize, Serialize};

use crate::models::state_diff::StateDiff;

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct StateDiffJson(pub BTreeMap<Address, BTreeMap<B256, [Option<B256>; 2]>>);

impl StateDiffJson {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&StateDiff> for StateDiffJson {
    fn from(state_diff: &StateDiff) -> Self {
        let mut result = BTreeMap::new();

        for (address, changes) in &state_diff.contracts {
            let mut slots = BTreeMap::new();
            for change in changes {
                slots.insert(change.slot, [change.value_before, change.value_after]);
            }
            result.insert(*address, slots);
        }

        StateDiffJson(result)
    }
}
