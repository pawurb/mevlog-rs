use alloy::providers::{
    Identity, RootProvider,
    fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
};

pub mod misc;
pub mod models;

pub type GenericProvider = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider,
>;

pub type RevmProvider = FillProvider<
    JoinFill<
        Identity,
        JoinFill<GasFiller, JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>>,
    >,
    RootProvider<alloy::network::AnyNetwork>,
    alloy::network::AnyNetwork,
>;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcUrlInfo {
    pub url: String,
    pub response_time_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainInfoJson {
    pub chain_id: u64,
    pub name: String,
    pub currency: String,
    pub explorer_url: Option<String>,
    pub rpc_timeout_ms: u64,
    pub rpc_urls: Vec<RpcUrlInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainInfoNoRpcsJson {
    pub chain_id: u64,
    pub name: String,
    pub currency: String,
    pub explorer_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainEntryJson {
    pub chain_id: u64,
    pub name: String,
    pub chain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explorer_url: Option<String>,
}
