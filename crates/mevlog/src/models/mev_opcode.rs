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
