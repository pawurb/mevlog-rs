use std::{path::PathBuf, str::FromStr, sync::Arc};

use alloy::{
    providers::{Provider, ProviderBuilder},
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
    db_actions::{check_and_create_indexes, db_file_exists},
    ens_utils::start_ens_lookup_worker,
    symbol_utils::{start_symbols_lookup_worker, SymbolLookupWorker},
};
use crate::{
    misc::db_actions::download_db_file,
    models::{db_chain::DBChain, evm_chain::EVMChain},
    GenericProvider,
};

pub struct SharedDeps {
    pub sqlite: SqlitePool,
    pub ens_lookup_worker: UnboundedSender<Address>,
    pub symbols_lookup_worker: SymbolLookupWorker,
    pub provider: Arc<GenericProvider>,
    pub chain: EVMChain,
}

pub async fn init_deps(shared_opts: &SharedOpts) -> Result<SharedDeps> {
    if shared_opts.rpc_url.is_none() {
        return Err(eyre::eyre!(
            "Missing provider URL, use --rpc-url or set ETH_RPC_URL env var"
        ));
    }

    if !db_file_exists() {
        let _ = std::fs::create_dir_all(config_path());
        println!("Database file missing");
        download_db_file().await?;
    } else {
        check_and_create_indexes().await?;
    }

    let sqlite_conn = sqlite_conn(None).await?;
    let ens_lookup_worker = start_ens_lookup_worker(shared_opts);
    let symbols_lookup_worker = start_symbols_lookup_worker(shared_opts);
    let provider = init_provider(shared_opts).await?;
    let provider = Arc::new(provider);

    let chain_id = provider.get_chain_id().await?;
    let db_chain = DBChain::find(chain_id as i64, &sqlite_conn)
        .await?
        .unwrap_or(DBChain::unknown(chain_id as i64));
    let chain = EVMChain::new(db_chain, shared_opts.rpc_url.clone().unwrap())?;

    Ok(SharedDeps {
        sqlite: sqlite_conn,
        ens_lookup_worker,
        symbols_lookup_worker,
        provider,
        chain,
    })
}

pub async fn init_provider(shared_opts: &SharedOpts) -> Result<GenericProvider> {
    let max_retry = 10;
    let backoff = 1000;
    let cups = 100;
    let retry_layer = RetryBackoffLayer::new(max_retry, backoff, cups);

    if let Some(rpc_url) = &shared_opts.rpc_url {
        debug!("Initializing HTTP provider");
        let client = RpcClient::builder()
            .layer(retry_layer)
            .http(rpc_url.parse()?);

        Ok(ProviderBuilder::new().connect_client(client))
    } else {
        unreachable!()
    }
}

pub fn config_path() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog")
}

#[derive(Clone, Debug, clap::Parser)]
pub struct SharedOpts {
    #[arg(long, help = "The URL of the HTTP provider", env = "ETH_RPC_URL")]
    pub rpc_url: Option<String>,

    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub trace: Option<TraceMode>,

    #[arg(long, help = "Show detailed tx calls info")]
    pub show_calls: bool,

    #[arg(long, help = "Display amounts in ERC20 Transfer event logs")]
    pub erc20_transfer_amount: bool,
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
