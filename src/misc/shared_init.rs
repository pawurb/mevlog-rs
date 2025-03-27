use std::{path::PathBuf, str::FromStr, sync::Arc};

use alloy::{
    providers::{Provider, ProviderBuilder, WsConnect},
    rpc::client::RpcClient,
    transports::layers::RetryBackoffLayer,
};
use eyre::Result;
use revm::primitives::Address;
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use super::{
    database::sqlite_conn,
    db_actions::db_file_exists,
    ens_utils::start_ens_lookup_worker,
    symbol_utils::{start_symbols_lookup_worker, SymbolLookupWorker},
};
use crate::{misc::db_actions::download_db_file, GenericProvider};

pub enum ProviderType {
    HTTP,
    WS,
}

#[derive(Debug, PartialEq)]
pub enum Chain {
    Mainnet,
    Base,
}

impl Chain {
    pub fn from_id(id: u64) -> Result<Self> {
        match id {
            1 => Ok(Self::Mainnet),
            8453 => Ok(Self::Base),
            _ => eyre::bail!("Invalid chain id {}", id),
        }
    }
}

pub struct SharedDeps {
    pub sqlite: SqlitePool,
    pub ens_lookup_worker: UnboundedSender<Address>,
    pub symbols_lookup_worker: SymbolLookupWorker,
    pub provider: Arc<GenericProvider>,
    pub provider_type: ProviderType,
    pub chain: Chain,
}

pub async fn init_deps(conn_opts: &ConnOpts) -> Result<SharedDeps> {
    if conn_opts.rpc_url.is_none() && conn_opts.ws_url.is_none() {
        return Err(eyre::eyre!(
            "Missing provider URL, use --rpc-url, --ws-url or set ETH_RPC_URL, ETH_WS_URL env vars"
        ));
    }

    if !db_file_exists() {
        let _ = std::fs::create_dir_all(config_path());
        println!("Database file missing");
        download_db_file().await?;
    }

    let sqlite_conn = sqlite_conn(None).await?;
    let ens_lookup_worker = start_ens_lookup_worker(conn_opts);
    let symbols_lookup_worker = start_symbols_lookup_worker(conn_opts);
    let (provider, provider_type) = init_provider(conn_opts).await?;
    let provider = Arc::new(provider);

    let chain_id = provider.get_chain_id().await?;
    let chain = Chain::from_id(chain_id)?;

    Ok(SharedDeps {
        sqlite: sqlite_conn,
        ens_lookup_worker,
        symbols_lookup_worker,
        provider,
        provider_type,
        chain,
    })
}

pub async fn init_provider(conn_opts: &ConnOpts) -> Result<(GenericProvider, ProviderType)> {
    let max_retry = 10;
    let backoff = 1000;
    let cups = 100;
    let retry_layer = RetryBackoffLayer::new(max_retry, backoff, cups);

    if let Some(ws_url) = &conn_opts.ws_url {
        debug!("Initializing WS provider");
        let ws = WsConnect::new(ws_url);
        let client = RpcClient::builder().layer(retry_layer).ws(ws).await?;
        Ok((ProviderBuilder::new().on_client(client), ProviderType::WS))
    } else if let Some(rpc_url) = &conn_opts.rpc_url {
        debug!("Initializing HTTP provider");
        let client = RpcClient::builder()
            .layer(retry_layer)
            .http(rpc_url.parse()?);

        Ok((ProviderBuilder::new().on_client(client), ProviderType::HTTP))
    } else {
        unreachable!()
    }
}

pub fn config_path() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog")
}

#[derive(Clone, Debug, clap::Parser)]
pub struct ConnOpts {
    #[arg(
        long,
        conflicts_with = "ws_url",
        help = "The URL of the HTTP provider",
        env = "ETH_RPC_URL"
    )]
    pub rpc_url: Option<String>,

    #[arg(
        long,
        conflicts_with = "rpc_url",
        help = "The URL of the WS provider",
        env = "ETH_WS_URL"
    )]
    pub ws_url: Option<String>,

    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub trace: Option<TraceMode>,
}

#[derive(Debug, Clone, clap::Parser)]
pub enum TraceMode {
    Revm,
    RPC,
}

impl FromStr for TraceMode {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "revm" => Ok(Self::Revm),
            "rpc" => Ok(Self::RPC),
            _ => Err(eyre::eyre!("Invalid tracing mode")),
        }
    }
}
