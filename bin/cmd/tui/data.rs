pub use mevlog::ChainEntryJson;
pub use mevlog::misc::shared_init::TraceMode;
pub use mevlog::models::json::mev_opcode_json::MEVOpcodeJson;
pub use mevlog::models::json::mev_transaction_json::MEVTransactionJson;
pub use mevlog::models::mev_transaction::CallExtract;

pub(crate) mod chains;
pub(crate) mod txs;
pub(crate) mod worker;

#[derive(Debug, Clone)]
pub(crate) struct RpcOpts {
    pub rpc_url: String,
    pub chain_id: u64,
    pub block_timeout_ms: u64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SearchFilters {
    pub blocks: String,
    pub position: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    pub event: Option<String>,
    pub not_event: Option<String>,
    pub method: Option<String>,
    pub erc20_transfer: Option<String>,
    pub tx_cost: Option<String>,
    pub gas_price: Option<String>,
}

impl SearchFilters {
    pub fn from_blocks(blocks: impl Into<String>) -> Self {
        Self {
            blocks: blocks.into(),
            ..Default::default()
        }
    }
}

#[allow(dead_code, clippy::large_enum_variant)]
pub(crate) enum DataRequest {
    Block(BlockId, RpcOpts),
    Tx(String, RpcOpts),
    Search(SearchFilters, RpcOpts),
    Chains(Option<String>),
    ChainInfo(String),
    Opcodes(String, TraceMode, RpcOpts),
    Traces(String, TraceMode, RpcOpts),
    TxTrace(String, TraceMode, RpcOpts),
    DetectTraceMode(String),
    RefreshRpc(u64, u64),
}

pub(crate) enum BlockId {
    Latest,
    Number(u64),
}

#[allow(dead_code, clippy::large_enum_variant)]
pub(crate) enum DataResponse {
    Block(u64, Vec<MEVTransactionJson>),
    SearchResults(Vec<MEVTransactionJson>),
    Chains(Vec<ChainEntryJson>),
    ChainInfo(ChainEntryJson),
    Opcodes(String, Vec<MEVOpcodeJson>),
    Traces(String, Vec<CallExtract>),
    TxTraced(String, MEVTransactionJson),
    TraceMode(TraceMode),
    RpcRefreshed(String),
    Error(String),
}
