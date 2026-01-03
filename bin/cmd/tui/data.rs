pub use mevlog::ChainEntryJson;
pub use mevlog::models::json::mev_transaction_json::MEVTransactionJson;

pub(crate) mod chains;
pub(crate) mod txs;
pub(crate) mod worker;

#[allow(dead_code)]
pub(crate) enum DataRequest {
    Block(BlockId),
    Tx(String),
    Chains(Option<String>),
}

pub(crate) enum BlockId {
    Latest,
    Number(u64),
}

#[allow(dead_code, clippy::large_enum_variant)]
pub(crate) enum DataResponse {
    Block(u64, Vec<MEVTransactionJson>),
    Tx(String, MEVTransactionJson),
    Chains(Vec<ChainEntryJson>),
    Error(String),
}
