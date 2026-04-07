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

use crate::{
    GenericProvider,
    misc::db_actions::download_db_file,
    models::{db_chain::DBChain, evm_chain::EVMChain},
};
use crate::{
    misc::{
        config::Config,
        database::sqlite_conn,
        db_actions::{check_and_create_indexes, db_file_exists},
        ens_utils::start_ens_lookup_worker,
        rpc_urls::get_chain_info,
        symbol_utils::{ERC20SymbolLookupWorker, start_symbols_lookup_worker},
    },
    models::json::mev_transaction_json::JsonSerializeOpts,
};

pub struct SharedDeps {
    pub sqlite: SqlitePool,
    pub ens_lookup_worker: UnboundedSender<Address>,
    pub symbols_lookup_worker: ERC20SymbolLookupWorker,
    pub provider: Arc<GenericProvider>,
    pub chain: Arc<EVMChain>,
    pub rpc_url: String,
}

pub struct ResolvedConn {
    pub provider: Arc<GenericProvider>,
    pub rpc_url: String,
    pub chain_id: u64,
}

pub async fn resolve_conn(conn_opts: &ConnOpts) -> Result<ResolvedConn> {
    Config::init_if_missing()?;
    let config = Config::load()?;

    let rpc_url = match (&conn_opts.rpc_url, conn_opts.chain_id) {
        (Some(url), _) => url.clone(),
        (None, Some(chain_id)) => {
            if let Some(chain_cfg) = config.get_chain(chain_id) {
                chain_cfg.rpc_url.clone()
            } else {
                let chain_info = get_chain_info(chain_id, conn_opts.rpc_timeout_ms, 1).await?;
                if chain_info.benchmarked_rpc_urls.is_empty() {
                    bail!("No working RPC URLs found for chain ID {}", chain_id)
                }
                chain_info.benchmarked_rpc_urls[0].0.clone()
            }
        }
        _ => {
            bail!("Either --rpc-url or --chain-id must be specified")
        }
    };

    let provider = init_provider(&rpc_url).await?;
    let provider = Arc::new(provider);

    let chain_id = match (conn_opts.rpc_url.as_ref(), conn_opts.chain_id) {
        (Some(_), Some(expected_chain_id)) => {
            if conn_opts.skip_verify_chain_id {
                expected_chain_id
            } else {
                let chain_id = provider.get_chain_id().await?;
                if chain_id != expected_chain_id {
                    bail!(
                        "Chain ID mismatch --chain-id {} != --chain-id from --rpc-url {chain_id}",
                        expected_chain_id,
                    );
                }
                chain_id
            }
        }
        (_, Some(chain_id)) => chain_id,
        (_, None) => provider.get_chain_id().await?,
    };

    Ok(ResolvedConn {
        provider,
        rpc_url,
        chain_id,
    })
}

#[hotpath::measure(future = true)]
pub async fn init_deps(conn_opts: &ConnOpts) -> Result<SharedDeps> {
    let resolved = resolve_conn(conn_opts).await?;

    if !db_file_exists() {
        let _ = std::fs::create_dir_all(config_path());
        println!("Database file missing");
        download_db_file().await?;
    }

    let sqlite = sqlite_conn(None).await?;
    check_and_create_indexes(&sqlite).await?;
    let ens_lookup_worker = start_ens_lookup_worker(&resolved.rpc_url);
    let symbols_lookup_worker = start_symbols_lookup_worker(&resolved.rpc_url);

    let db_chain = DBChain::find(resolved.chain_id as i64, &sqlite)
        .await?
        .unwrap_or(DBChain::unknown(resolved.chain_id as i64));
    let chain = Arc::new(EVMChain::new(db_chain, resolved.rpc_url.clone())?);

    Ok(SharedDeps {
        sqlite,
        ens_lookup_worker,
        symbols_lookup_worker,
        provider: resolved.provider,
        chain,
        rpc_url: resolved.rpc_url,
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

pub fn mevlog_cmd_path() -> PathBuf {
    std::env::var_os("MEVLOG_CMD_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("mevlog"))
}

#[derive(Clone, Debug, clap::Parser)]
pub struct SharedOpts {
    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub evm_trace: Option<TraceMode>,

    #[arg(long, help = "Show detailed tx calls info")]
    pub evm_calls: bool,

    #[arg(long, help = "Display EVM opcodes executed by the transaction")]
    pub evm_ops: bool,

    #[arg(
        long,
        help = "Display storage slot changes (state diff) for the transaction"
    )]
    pub evm_state_diff: bool,

    #[arg(long, help = "Display amounts in ERC20 Transfer event logs")]
    pub erc20_transfer_amount: bool,

    #[arg(long, help = "Enable ENS domains lookup")]
    pub ens: bool,

    #[arg(long, help = "Enable ERC20 symbols lookup")]
    pub erc20_symbols: bool,

    #[arg(long, help = "Include event logs in output")]
    pub logs: bool,

    #[arg(
        long,
        help = "Provide native token price in USD instead of reading it from price oracle"
    )]
    pub native_token_price: Option<f64>,
}

impl SharedOpts {
    pub fn json_serialize_opts(&self) -> JsonSerializeOpts {
        JsonSerializeOpts {
            include_logs: self.logs,
            include_evm_calls: self.evm_calls,
            include_evm_opcodes: self.evm_ops,
            include_evm_state_diff: self.evm_state_diff,
        }
    }
}

#[derive(Clone, Debug, clap::Parser)]
pub struct ConnOpts {
    #[arg(long, help = "The URL of the HTTP provider")]
    pub rpc_url: Option<String>,

    #[arg(long, help = "Chain ID to automatically select RPC URL from ChainList")]
    pub chain_id: Option<u64>,

    #[arg(
        long,
        help = "Timeout in milliseconds for filtering RPC URLs",
        default_value = "1000"
    )]
    pub rpc_timeout_ms: u64,

    #[arg(
        long,
        help = "Timeout in milliseconds for block fetching",
        default_value = "10000"
    )]
    pub block_timeout_ms: u64,

    #[arg(long, help = "Skip verifying --chain-id with data from --rpc-url")]
    pub skip_verify_chain_id: bool,
}

#[derive(Debug, Clone, clap::Parser, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TraceMode {
    Revm,
    #[serde(rename = "rpc")]
    RPC,
}

impl std::fmt::Display for TraceMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Revm => write!(f, "revm"),
            Self::RPC => write!(f, "rpc"),
        }
    }
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
    Json,
    JsonPretty,
}

impl FromStr for OutputFormat {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "json" => Ok(Self::Json),
            "json-pretty" => Ok(Self::JsonPretty),
            _ => Err(eyre::eyre!("Invalid output format")),
        }
    }
}
