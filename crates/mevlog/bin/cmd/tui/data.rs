pub(super) use mevlog::ChainEntryJson;
pub(super) use mevlog::misc::shared_init::ConnOpts;
pub(super) use mevlog::models::json::log_json::LogJson;
pub(super) use mevlog::models::json::transaction_json::TransactionJson;

pub(crate) mod chains;
pub(crate) mod txs;
pub(crate) mod worker;

/// Builds the connection options the in-process `cmds` calls run with. The TUI
/// always has a concrete RPC URL, so it is passed verbatim (chain ID is only a
/// fallback for RPC-URL selection and is unused here); the remaining fields take
/// their CLI defaults.
pub(crate) fn conn_opts(rpc_url: String) -> ConnOpts {
    ConnOpts {
        rpc_url: Some(rpc_url),
        chain_id: None,
        rpc_timeout_ms: 1000,
        block_timeout_ms: 10000,
        skip_verify_chain_id: false,
        txs_db_dir: None,
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RpcOpts {
    pub rpc_url: String,
    pub block_timeout_ms: u64,
}

#[allow(dead_code, clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum DataRequest {
    Block(BlockId, RpcOpts),
    Tx(String, RpcOpts),
    Chains(Option<String>),
    ChainInfo(String),
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
    RpcRefreshed(String),
    Error(String),
}
