use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

/// JSON representation of a single log/event row.
///
/// Deserialization contract for the logs embedded in `mevlog tx --logs`, which
/// are rendered in SQL (see [`logs_display_query`]); `topic0..topic3` are folded
/// into `topics` by the command.
///
/// [`logs_display_query`]: crate::db::txs::display_sql::logs_display_query
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LogJson {
    pub log_index: u64,
    pub address: Address,
    /// Resolved event signature, `None` when it could not be resolved.
    pub signature: Option<String>,
    pub topics: Vec<FixedBytes<32>>,
    /// Raw log data as `0x`-hex.
    pub data: String,
    /// Decoded ERC20 transfer amount as a decimal string, `None` for non-transfer logs.
    pub erc20_amount: Option<String>,
}
