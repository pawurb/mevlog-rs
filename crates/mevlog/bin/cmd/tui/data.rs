use tokio::process::Command;

pub use mevlog::ChainEntryJson;
pub use mevlog::misc::shared_init::{TraceMode, mevlog_cmd_path};
pub use mevlog::models::call_extract::CallExtract;
pub use mevlog::models::json::log_json::LogJson;
pub use mevlog::models::json::state_diff_json::StateDiffJson;
pub use mevlog::models::json::transaction_json::TransactionJson;

pub(crate) mod chains;
pub(crate) mod txs;
pub(crate) mod worker;

pub(crate) fn mevlog_cmd() -> Command {
    Command::new(mevlog_cmd_path())
}

#[derive(Debug, Clone)]
pub(crate) struct RpcOpts {
    pub rpc_url: String,
    pub chain_id: u64,
    pub block_timeout_ms: u64,
}

#[allow(dead_code, clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum DataRequest {
    Block(BlockId, RpcOpts),
    Tx(String, RpcOpts),
    Chains(Option<String>),
    ChainInfo(String),
    Traces(String, TraceMode, RpcOpts),
    StateDiff(String, TraceMode, RpcOpts),
    TxTrace(String, TraceMode, RpcOpts),
    DetectTraceMode(String),
    RefreshRpc(u64, u64),
}

#[derive(Debug)]
pub(crate) enum BlockId {
    Latest,
    Number(u64),
}

#[allow(dead_code, clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum DataResponse {
    Block(u64, Vec<TransactionJson>),
    Chains(Vec<ChainEntryJson>),
    ChainInfo(ChainEntryJson),
    Traces(String, Vec<CallExtract>),
    StateDiff(String, StateDiffJson),
    TxTraced(String, TransactionJson),
    TraceMode(TraceMode),
    RpcRefreshed(String),
    Error(String),
}
