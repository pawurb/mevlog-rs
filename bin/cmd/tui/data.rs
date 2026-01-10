pub use mevlog::ChainEntryJson;
pub use mevlog::misc::shared_init::TraceMode;
pub use mevlog::models::json::mev_opcode_json::MEVOpcodeJson;
pub use mevlog::models::json::mev_transaction_json::MEVTransactionJson;
pub use mevlog::models::mev_transaction::CallExtract;

pub(crate) mod chains;
pub(crate) mod txs;
pub(crate) mod worker;

#[allow(dead_code)]
pub(crate) enum DataRequest {
    Block(BlockId),
    Tx(String),
    Chains(Option<String>),
    ChainInfo(String),
    Opcodes(String, TraceMode),
    Traces(String, TraceMode),
    DetectTraceMode(String),
    ResolveRpcUrl(u64),
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
    ChainInfo(ChainEntryJson),
    Opcodes(String, Vec<MEVOpcodeJson>),
    Traces(String, Vec<CallExtract>),
    TraceMode(TraceMode),
    RpcUrl(u64, String),
    Error(String),
}
