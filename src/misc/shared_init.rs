use std::{path::PathBuf, str::FromStr, sync::Arc};

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::RpcClient,
    transports::layers::RetryBackoffLayer,
};
use eyre::Result;
use revm::primitives::{address, Address};
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

#[derive(Debug, PartialEq, Clone)]
pub enum EVMChainType {
    Mainnet,
    Base,
    BSC,
    Arbitrum,
}

#[derive(Debug, Clone)]
pub struct EVMChain {
    pub chain_type: EVMChainType,
    pub rpc_url: String,
}

impl EVMChainType {
    pub fn chain_id(&self) -> u64 {
        match self {
            EVMChainType::Mainnet => 1,
            EVMChainType::Base => 8453,
            EVMChainType::BSC => 56,
            EVMChainType::Arbitrum => 42161,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            EVMChainType::Mainnet => "mainnet",
            EVMChainType::Base => "base",
            EVMChainType::BSC => "bsc",
            EVMChainType::Arbitrum => "arbitrum",
        }
    }

    pub fn supported() -> Vec<Self> {
        vec![
            EVMChainType::Mainnet,
            EVMChainType::Base,
            EVMChainType::BSC,
            EVMChainType::Arbitrum,
        ]
    }

    pub fn supported_chains_text() -> String {
        let chains = Self::supported()
            .iter()
            .map(|chain| format!("- {} ({})", chain.name(), chain.chain_id()))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Currently supported EVM chains:
{chains}
Visit https://github.com/pawurb/mevlog-rs/issues/9 to add more."#
        )
    }
}

impl EVMChain {
    pub fn new(chain_id: u64, rpc_url: String) -> Result<Self> {
        let supported_chains = EVMChainType::supported();
        if !supported_chains
            .iter()
            .any(|chain| chain.chain_id() == chain_id)
        {
            eyre::bail!(
                "Invalid chain id {}. {}",
                chain_id,
                EVMChainType::supported_chains_text()
            )
        }

        let matching_chain = supported_chains
            .iter()
            .find(|chain| chain.chain_id() == chain_id)
            .unwrap();

        Ok(Self {
            rpc_url,
            chain_type: matching_chain.clone(),
        })
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_type.chain_id()
    }

    pub fn name(&self) -> &str {
        self.chain_type.name()
    }

    pub fn revm_cache_dir_name(&self) -> &str {
        self.name()
    }

    pub fn cryo_cache_dir_name(&self) -> &str {
        match self.chain_type {
            EVMChainType::Mainnet => "ethereum",
            _ => self.name(),
        }
    }

    pub fn price_oracle(&self) -> Address {
        match self.chain_type {
            EVMChainType::Mainnet => address!("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"),
            EVMChainType::Base => address!("0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"),
            EVMChainType::BSC => address!("0x0567f2323251f0aab15c8dfb1967e4e8a7d42aee"),
            EVMChainType::Arbitrum => address!("0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612"),
        }
    }

    pub fn etherscan_url(&self) -> &str {
        match self.chain_type {
            EVMChainType::Mainnet => "https://etherscan.io",
            EVMChainType::Base => "https://basescan.org",
            EVMChainType::BSC => "https://bscscan.com",
            EVMChainType::Arbitrum => "https://arbiscan.io",
        }
    }

    pub fn is_optimism(&self) -> bool {
        self.chain_type == EVMChainType::Base
    }

    pub fn currency_symbol(&self) -> &str {
        match self.chain_type {
            EVMChainType::BSC => "BNB",
            _ => "ETH",
        }
    }
}

pub struct SharedDeps {
    pub sqlite: SqlitePool,
    pub ens_lookup_worker: UnboundedSender<Address>,
    pub symbols_lookup_worker: SymbolLookupWorker,
    pub provider: Arc<GenericProvider>,
    pub chain: EVMChain,
}

pub async fn init_deps(conn_opts: &ConnOpts) -> Result<SharedDeps> {
    if conn_opts.rpc_url.is_none() {
        return Err(eyre::eyre!(
            "Missing provider URL, use --rpc-url or set ETH_RPC_URL env var"
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
    let provider = init_provider(conn_opts).await?;
    let provider = Arc::new(provider);

    let chain_id = provider.get_chain_id().await?;
    let chain = EVMChain::new(chain_id, conn_opts.rpc_url.clone().unwrap())?;

    Ok(SharedDeps {
        sqlite: sqlite_conn,
        ens_lookup_worker,
        symbols_lookup_worker,
        provider,
        chain,
    })
}

pub async fn init_provider(conn_opts: &ConnOpts) -> Result<GenericProvider> {
    let max_retry = 10;
    let backoff = 1000;
    let cups = 100;
    let retry_layer = RetryBackoffLayer::new(max_retry, backoff, cups);

    if let Some(rpc_url) = &conn_opts.rpc_url {
        debug!("Initializing HTTP provider");
        let client = RpcClient::builder()
            .layer(retry_layer)
            .http(rpc_url.parse()?);

        Ok(ProviderBuilder::new().on_client(client))
    } else {
        unreachable!()
    }
}

pub fn config_path() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog")
}

#[derive(Clone, Debug, clap::Parser)]
pub struct ConnOpts {
    #[arg(long, help = "The URL of the HTTP provider", env = "ETH_RPC_URL")]
    pub rpc_url: Option<String>,

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
