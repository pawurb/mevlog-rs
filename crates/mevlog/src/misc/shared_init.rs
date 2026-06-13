use std::{path::PathBuf, str::FromStr, sync::Arc};

use alloy::{
    providers::{Provider, ProviderBuilder},
    rpc::client::RpcClient,
    transports::layers::RetryBackoffLayer,
};
use eyre::{Result, bail};
use sqlx::SqlitePool;
use tracing::debug;

use crate::misc::{
    config::{Config, CustomTable},
    rpc_urls::get_chain_info,
};
use crate::{
    GenericProvider,
    db::{
        sigs::{
            self,
            actions::{check_and_create_indexes, download_file, file_exists},
            models::chain::Chain,
        },
        txs,
    },
    models::evm_chain::EVMChain,
};

pub struct SharedDeps {
    pub sqlite: SqlitePool,
    pub txs: SqlitePool,
    /// File path of the `txs` DB, used by `run_raw_query` to open a read-only
    /// `rusqlite` connection for `--sql` queries.
    pub txs_read_path: String,
    pub provider: Arc<GenericProvider>,
    pub chain: Arc<EVMChain>,
    pub rpc_url: String,
    /// Config-defined custom tables applicable to this chain, already synced
    /// into the txs DB; the indexing path populates them per chunk.
    pub(crate) custom_tables: Vec<CustomTable>,
}

impl SharedDeps {
    /// Names of the custom tables to allowlist for `--sql` reads.
    pub(crate) fn custom_table_names(&self) -> Vec<String> {
        self.custom_tables.iter().map(|t| t.name.clone()).collect()
    }
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

    if !file_exists() {
        let _ = std::fs::create_dir_all(config_path());
        eprintln!("Database file missing");
        download_file().await?;
    }

    let sqlite = sigs::conn(None).await?;
    check_and_create_indexes(&sqlite).await?;

    // `--txs-db-dir` overrides only the directory; the filename keeps the
    // `mevlog-txs-v{N}-{chain_id}.db` convention.
    let txs_db_url = conn_opts.txs_db_dir.as_ref().map(|dir| {
        PathBuf::from(dir)
            .join(txs::db_file_name(txs::SCHEMA_VERSION, resolved.chain_id))
            .to_string_lossy()
            .into_owned()
    });
    txs::init_db(txs_db_url.clone(), resolved.chain_id).await?;
    let txs = txs::conn(txs_db_url.clone(), resolved.chain_id, false).await?;

    let config = Config::load()?;
    let custom_tables =
        txs::custom_tables::sync(&config.custom_tables()?, resolved.chain_id, &txs).await?;
    let txs_read_path = txs_db_url.unwrap_or_else(|| {
        txs::default_db_path(resolved.chain_id)
            .to_string_lossy()
            .into_owned()
    });

    let db_chain = Chain::find(resolved.chain_id as i64, &sqlite)
        .await?
        .unwrap_or(Chain::unknown(resolved.chain_id as i64));
    let chain = Arc::new(EVMChain::new(db_chain, resolved.rpc_url.clone())?);

    Ok(SharedDeps {
        sqlite,
        txs,
        txs_read_path,
        provider: resolved.provider,
        chain,
        rpc_url: resolved.rpc_url,
        custom_tables,
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

pub(crate) fn config_path() -> PathBuf {
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

    #[arg(
        long,
        help = "Provide native token price in USD instead of reading it from price oracle"
    )]
    pub native_token_price: Option<f64>,
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

    #[arg(
        long,
        help = "Override the directory holding the per-chain transactions SQLite DB (mainly for tests); filename stays mevlog-txs-v{N}-{chain_id}.db"
    )]
    pub txs_db_dir: Option<String>,
}

const DEFAULT_CRYO_REQUESTS_PER_SECOND: u64 = 25;
const DEFAULT_CRYO_MAX_CONCURRENT_REQUESTS: u64 = 10;
const DEFAULT_CRYO_MAX_RETRIES: u64 = 8;
const DEFAULT_CRYO_INITIAL_BACKOFF_MS: u64 = 1000;

#[derive(Clone, Debug, clap::Parser)]
pub struct CryoOpts {
    #[arg(
        long,
        help = "Max RPC requests per second for cryo block fetching",
        default_value_t = DEFAULT_CRYO_REQUESTS_PER_SECOND
    )]
    pub cryo_requests_per_second: u64,

    #[arg(
        long,
        help = "Max concurrent RPC requests for cryo block fetching",
        default_value_t = DEFAULT_CRYO_MAX_CONCURRENT_REQUESTS
    )]
    pub cryo_max_concurrent_requests: u64,

    #[arg(
        long,
        help = "Max retries for cryo RPC provider errors",
        default_value_t = DEFAULT_CRYO_MAX_RETRIES
    )]
    pub cryo_max_retries: u64,

    #[arg(
        long,
        help = "Initial retry backoff in milliseconds for cryo RPC provider errors",
        default_value_t = DEFAULT_CRYO_INITIAL_BACKOFF_MS
    )]
    pub cryo_initial_backoff: u64,
}

impl Default for CryoOpts {
    fn default() -> Self {
        Self {
            cryo_requests_per_second: DEFAULT_CRYO_REQUESTS_PER_SECOND,
            cryo_max_concurrent_requests: DEFAULT_CRYO_MAX_CONCURRENT_REQUESTS,
            cryo_max_retries: DEFAULT_CRYO_MAX_RETRIES,
            cryo_initial_backoff: DEFAULT_CRYO_INITIAL_BACKOFF_MS,
        }
    }
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
    Csv,
    Table,
}

impl FromStr for OutputFormat {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "json" => Ok(Self::Json),
            "json-pretty" => Ok(Self::JsonPretty),
            "csv" => Ok(Self::Csv),
            "table" => Ok(Self::Table),
            _ => Err(eyre::eyre!("Invalid output format")),
        }
    }
}
