use revm::primitives::{Address, FixedBytes, U256};

/// Basic SQLite-backed transaction record.
///
/// Holds only the core transaction + receipt fields. Logs/events and EVM traces
/// are intentionally excluded and will be stored in separate tables later.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    pub block_number: u64,
    pub tx_index: u64,
    pub tx_hash: FixedBytes<32>,
    pub nonce: u64,
    pub from_address: Address,
    /// `None` for contract-creation transactions.
    pub to_address: Option<Address>,
    pub value: U256,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub effective_gas_price: u128,
    pub gas_price: u128,
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
    pub transaction_type: Option<u8>,
    pub success: bool,
    pub chain_id: u64,
    /// First 4 bytes of the calldata (function selector).
    /// `None` for contract-creation transactions or calldata shorter than 4 bytes.
    pub signature_hash: Option<FixedBytes<4>>,
    /// `None` when the method signature could not be resolved.
    pub signature: Option<String>,
}
