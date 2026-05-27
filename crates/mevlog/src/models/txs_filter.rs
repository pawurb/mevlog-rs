use std::collections::HashSet;

use crate::misc::args_parsing::PositionRange;

/// Options controlling which txs are collected and how much per-tx data is
/// traced/displayed. No content filtering happens here anymore — filtering will
/// be handled by querying the local SQLite store.
#[derive(Debug, Default)]
pub struct TxsFilter {
    pub tx_indexes: Option<HashSet<u64>>,
    pub tx_position: Option<PositionRange>,
    pub reversed_order: bool,
    pub top_metadata: bool,
    pub show_calls: bool,
    pub show_opcodes: bool,
    pub show_state_diff: bool,
}
