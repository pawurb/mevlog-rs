use std::collections::BTreeMap;

use alloy::primitives::{Address, B256};
use serde::{Deserialize, Serialize};

use crate::models::mev_state_diff::MEVStateDiff;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MEVStateDiffJson(pub BTreeMap<Address, BTreeMap<B256, [Option<B256>; 2]>>);

impl From<&MEVStateDiff> for MEVStateDiffJson {
    fn from(state_diff: &MEVStateDiff) -> Self {
        let mut result = BTreeMap::new();

        for (address, changes) in &state_diff.contracts {
            let mut slots = BTreeMap::new();
            for change in changes {
                slots.insert(change.slot, [change.value_before, change.value_after]);
            }
            result.insert(*address, slots);
        }

        MEVStateDiffJson(result)
    }
}
