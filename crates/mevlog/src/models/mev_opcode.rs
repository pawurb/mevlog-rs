use std::fmt;

#[derive(Clone, Debug)]
pub struct MEVOpcode {
    pub pc: u64,
    pub op: String,
    pub cost: u64,
    pub gas_left: u64,
}

impl MEVOpcode {
    pub fn new(pc: u64, op: String, cost: u64, gas_left: u64) -> Self {
        Self {
            pc,
            op,
            cost,
            gas_left,
        }
    }
}

impl fmt::Display for MEVOpcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<8} {:<16} {:<8} {:<10}",
            self.pc, self.op, self.cost, self.gas_left
        )
    }
}
