use serde::{Deserialize, Serialize};

use crate::models::mev_opcode::MEVOpcode;

#[derive(Clone, Serialize, Deserialize)]
pub struct MEVOpcodeJson {
    pub pc: u64,
    pub op: String,
    pub cost: u64,
    pub gas_left: u64,
}

impl From<&MEVOpcode> for MEVOpcodeJson {
    fn from(opcode: &MEVOpcode) -> Self {
        Self {
            pc: opcode.pc,
            op: opcode.op.clone(),
            cost: opcode.cost,
            gas_left: opcode.gas_left,
        }
    }
}
