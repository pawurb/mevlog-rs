use alloy::providers::{
    fillers::{BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill, NonceFiller},
    Identity, RootProvider,
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

pub type RevmProvider = alloy::providers::fillers::FillProvider<
    alloy::providers::fillers::JoinFill<
        alloy::providers::Identity,
        alloy::providers::fillers::JoinFill<
            alloy::providers::fillers::GasFiller,
            alloy::providers::fillers::JoinFill<
                alloy::providers::fillers::BlobGasFiller,
                alloy::providers::fillers::JoinFill<
                    alloy::providers::fillers::NonceFiller,
                    alloy::providers::fillers::ChainIdFiller,
                >,
            >,
        >,
    >,
    alloy::providers::RootProvider<alloy::network::AnyNetwork>,
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
