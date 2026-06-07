use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

/// JSON representation of a single block's metadata; the deserialization contract
/// for the `mevlog block` output (rendered by [`block_display_query`]).
///
/// [`block_display_query`]: crate::db::txs::display_sql::block_display_query
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct BlockJson {
    pub block_number: u64,
    pub block_hash: FixedBytes<32>,
    /// Fee recipient (cryo `author`).
    pub miner: Address,
    pub gas_used: u64,
    /// Unix timestamp (seconds).
    pub timestamp: u64,
    /// `None` for pre-EIP-1559 blocks.
    pub base_fee_per_gas: Option<u64>,
    /// Base fee formatted in gwei, `None` when there is no base fee.
    pub display_base_fee_per_gas: Option<String>,
    /// Number of indexed transactions in the block.
    pub txs_count: u64,
}
