use serde::Deserialize;
mod fetcher;
pub(crate) mod worker;

/// Transaction data parsed from mevlog CLI JSON output
#[derive(Debug, Clone, Deserialize)]
pub struct TxRow {
    pub block_number: u64,
    pub tx_hash: String,
    pub from: String,
    pub to: Option<String>,
    pub display_value: String,
    pub gas_price: u128,
    pub success: bool,
}

pub(crate) enum DataRequest {
    FetchBlock(u64),
    FetchTx(String),
}

pub(crate) enum DataResponse {
    Block(u64, Vec<TxRow>),
    Tx(String, TxRow),
}
