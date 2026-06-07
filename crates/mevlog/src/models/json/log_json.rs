use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

/// JSON representation of a single log/event row; the deserialization contract for
/// the `mevlog tx-logs` output (rendered by [`logs_display_query`]) and the
/// `mevlog block-logs` output (rendered by [`block_logs_display_query`]).
///
/// `topic0..topic3` are kept as separate columns (`None` when absent), faithful to
/// the echoed SQL; `topic0` is the event signature hash.
///
/// `tx_index` is only present in `block-logs` output (where rows span multiple
/// transactions and must be grouped by tx); it is `None` for `tx-logs`.
///
/// [`logs_display_query`]: crate::db::txs::display_sql::logs_display_query
/// [`block_logs_display_query`]: crate::db::txs::display_sql::block_logs_display_query
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct LogJson {
    #[serde(default)]
    pub tx_index: Option<u64>,
    pub log_index: u64,
    pub address: Address,
    /// Resolved event signature, `None` when it could not be resolved.
    pub signature: Option<String>,
    pub topic0: Option<FixedBytes<32>>,
    pub topic1: Option<FixedBytes<32>>,
    pub topic2: Option<FixedBytes<32>>,
    pub topic3: Option<FixedBytes<32>>,
    /// Raw log data as `0x`-hex.
    pub data: String,
    /// Decoded ERC20 transfer amount as a decimal string, `None` for non-transfer logs.
    pub erc20_amount: Option<String>,
}
