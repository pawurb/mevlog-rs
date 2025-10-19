pub use mevlog::models::json::mev_transaction_json::MEVTransactionJson;

mod fetcher;
pub(crate) mod worker;

#[allow(dead_code)]
pub(crate) enum DataRequest {
    Block(BlockId),
    Tx(String),
}

pub(crate) enum BlockId {
    Latest,
    Number(u64),
}

#[allow(dead_code, clippy::large_enum_variant)]
pub(crate) enum DataResponse {
    Block(u64, Vec<MEVTransactionJson>),
    Tx(String, MEVTransactionJson),
    Error(String),
}
