use std::{path::PathBuf, str::FromStr, sync::Arc};

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::RpcClient,
    transports::layers::RetryBackoffLayer,
};
use eyre::{Result, bail};
use revm::primitives::Address;
use sqlx::SqlitePool;
use tokio::sync::mpsc::UnboundedSender;
use tracing::debug;

use super::{
    database::sqlite_conn,
    db_actions::{check_and_create_indexes, db_file_exists},
    ens_utils::start_ens_lookup_worker,
    rpc_urls::get_chain_info,
    symbol_utils::{ERC20SymbolLookupWorker, start_symbols_lookup_worker},
};
use crate::{
    GenericProvider,
    misc::db_actions::download_db_file,
    models::{db_chain::DBChain, evm_chain::EVMChain},
};

pub struct SharedDeps {
    pub sqlite: SqlitePool,
    pub ens_lookup_worker: UnboundedSender<Address>,
    pub symbols_lookup_worker: ERC20SymbolLookupWorker,
    pub provider: Arc<GenericProvider>,
    pub chain: EVMChain,
    pub rpc_url: String,
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
pub async fn init_deps(conn_opts: &ConnOpts) -> Result<SharedDeps> {
    let rpc_url = match (&conn_opts.rpc_url, conn_opts.chain_id) {
        (Some(url), Some(_)) => url.clone(),
        (Some(url), None) => url.clone(),
        (None, Some(chain_id)) => {
            let chain_info = get_chain_info(chain_id, conn_opts.rpc_timeout_ms, 1).await?;
            if chain_info.benchmarked_rpc_urls.is_empty() {
                bail!("No working RPC URLs found for chain ID {}", chain_id)
            }
            chain_info.benchmarked_rpc_urls[0].0.clone()
        }
        _ => {
            bail!("Either --rpc-url or --chain-id must be specified")
        }
    };

    if !db_file_exists() {
        let _ = std::fs::create_dir_all(config_path());
        println!("Database file missing");
        download_db_file().await?;
    }

    let sqlite = sqlite_conn(None).await?;
    check_and_create_indexes(&sqlite).await?;
    let ens_lookup_worker = start_ens_lookup_worker(&rpc_url);
    let symbols_lookup_worker = start_symbols_lookup_worker(&rpc_url);
    let provider = init_provider(&rpc_url).await?;
    let provider = Arc::new(provider);

    let chain_id = if conn_opts.rpc_url.is_some() && conn_opts.chain_id.is_some() {
        if conn_opts.skip_verify_chain_id {
            conn_opts.chain_id.unwrap()
        } else {
            let chain_id = provider.get_chain_id().await?;

            if chain_id != conn_opts.chain_id.unwrap() {
                bail!(
                    "Chain ID mismatch --chain-id {} != --chain-id from --rpc-url {chain_id}",
                    conn_opts.chain_id.unwrap(),
                );
            }
            chain_id
        }
    } else if conn_opts.chain_id.is_some() {
        conn_opts.chain_id.unwrap()
    } else {
        provider.get_chain_id().await?
    };

    let db_chain = DBChain::find(chain_id as i64, &sqlite)
        .await?
        .unwrap_or(DBChain::unknown(chain_id as i64));
    let chain = EVMChain::new(db_chain, rpc_url.clone())?;

    Ok(SharedDeps {
        sqlite,
        ens_lookup_worker,
        symbols_lookup_worker,
        provider,
        chain,
        rpc_url,
    })
}

pub async fn init_provider(rpc_url: &str) -> Result<GenericProvider> {
    let max_retry = 10;
    let backoff = 1000;
    let cups = 100;
    let retry_layer = RetryBackoffLayer::new(max_retry, backoff, cups);

    debug!("Initializing HTTP provider");
    let client = RpcClient::builder()
        .layer(retry_layer)
        .http(rpc_url.parse()?);

    Ok(ProviderBuilder::new().connect_client(client))
}

pub fn config_path() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog")
}

#[derive(Clone, Debug, clap::Parser)]
pub struct SharedOpts {
    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub trace: Option<TraceMode>,

    #[arg(long, help = "Show detailed tx calls info")]
    pub show_calls: bool,

    #[arg(long, help = "Display amounts in ERC20 Transfer event logs")]
    pub erc20_transfer_amount: bool,

    #[arg(long, help = "Enable ENS domains lookup")]
    pub ens: bool,

    #[arg(long, help = "Enable ERC20 symbols lookup")]
    pub erc20_symbols: bool,

    #[arg(
        long,
        help = "Provide native token price in USD instead of reading it from price oracle"
    )]
    pub native_token_price: Option<f64>,
}

#[derive(Clone, Debug, clap::Parser)]
pub struct ConnOpts {
    #[arg(long, help = "The URL of the HTTP provider", env = "ETH_RPC_URL")]
    pub rpc_url: Option<String>,

    #[arg(long, help = "Chain ID to automatically select RPC URL from ChainList")]
    pub chain_id: Option<u64>,

    #[arg(
        long,
        help = "Timeout in milliseconds for filtering RPC URLs",
        default_value = "1000"
    )]
    pub rpc_timeout_ms: u64,

    #[arg(long, help = "Skip verifying --chain-id with data from --rpc-url")]
    pub skip_verify_chain_id: bool,
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

#[derive(Debug, Clone, clap::ValueEnum, PartialEq)]
pub enum OutputFormat {
    Text,
    Json,
    JsonPretty,
    JsonStream,
    JsonPrettyStream,
}

impl OutputFormat {
    pub fn is_stream(&self) -> bool {
        self == &Self::JsonStream || self == &Self::JsonPrettyStream || self == &Self::Text
    }

    pub fn non_stream_json(&self) -> bool {
        self == &Self::Json || self == &Self::JsonPretty
    }
}

impl FromStr for OutputFormat {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "text" => Ok(Self::Text),
            "json" => Ok(Self::Json),
            "json-pretty" => Ok(Self::JsonPretty),
            "json-stream" => Ok(Self::JsonStream),
            "json-pretty-stream" => Ok(Self::JsonPrettyStream),
            _ => Err(eyre::eyre!("Invalid output format")),
        }
    }
}
