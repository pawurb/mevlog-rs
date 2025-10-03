use std::{collections::HashMap, path::PathBuf, sync::Arc};

use alloy::{providers::Provider, sol};
use eyre::Result;
use revm::primitives::Address;
use tokio::sync::{
    RwLock,
    mpsc::{self, UnboundedSender},
};

use super::shared_init::init_provider;
use crate::{GenericProvider, models::mev_log_signature::MEVLogSignatureType};

sol! {
  #[sol(rpc)]
  interface IERC20 {
    function symbol() external view returns (string memory);
  }

  #[sol(rpc)]
  interface IUniswapPair {
    function token0() external view returns (address);
    function token1() external view returns (address);
  }
}

const MISSING_SYMBOL: &str = "E";

static SYMBOLS_MEMORY_CACHE: std::sync::LazyLock<RwLock<HashMap<Address, CachedEntry>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Clone, Debug)]
pub enum CachedEntry {
    Known(String),
    KnownEmpty,
    Unknown,
}

pub enum ERC20SymbolsLookup {
    Async(ERC20SymbolLookupWorker),
    OnlyCached,
}

impl ERC20SymbolsLookup {
    pub fn lookup_mode(
        erc20_symbold_lookup_worker: ERC20SymbolLookupWorker,
        erc20_symbols_enabled: bool,
    ) -> ERC20SymbolsLookup {
        if erc20_symbols_enabled {
            ERC20SymbolsLookup::Async(erc20_symbold_lookup_worker)
        } else {
            ERC20SymbolsLookup::OnlyCached
        }
    }
}

pub type ERC20SymbolLookupWorker = UnboundedSender<(Address, MEVLogSignatureType)>;

pub fn start_symbols_lookup_worker(rpc_url: &str) -> ERC20SymbolLookupWorker {
    let (tx, mut rx) = mpsc::unbounded_channel::<(Address, MEVLogSignatureType)>();

    let rpc_url = rpc_url.to_string();
    tokio::spawn(async move {
        let provider = init_provider(&rpc_url).await.unwrap();
        let provider = Arc::new(provider);
        while let Some(data) = rx.recv().await {
            let target = data.0;
            let sig_type = data.1;

            match sig_type {
                MEVLogSignatureType::ERC20 => match get_erc20_symbol(target, &provider).await {
                    Ok(_) => (),
                    Err(e) => {
                        tracing::error!("Error looking up ERC20 symbol: {}", e);
                    }
                },
                MEVLogSignatureType::UNIV2 | MEVLogSignatureType::UNIV3 => {
                    match get_uniswap_symbol(target, &provider).await {
                        Ok(_) => (),
                        Err(e) => {
                            tracing::error!("Error looking up UNI symbol: {}", e);
                        }
                    }
                }
            }
        }
    });

    tx
}

async fn get_uniswap_symbol(target: Address, provider: &Arc<GenericProvider>) -> Result<()> {
    match get_uniswap_symbol_core(target, provider).await {
        Ok(_) => (),
        Err(_e) => {
            write_symbol_cache(target, None).await?;
        }
    }
    Ok(())
}

async fn get_uniswap_symbol_core(target: Address, provider: &Arc<GenericProvider>) -> Result<()> {
    let pair = IUniswapPair::new(target, provider.clone());

    let multicall = provider.multicall().add(pair.token0()).add(pair.token1());

    let (token0, token1) = multicall.aggregate().await?;

    let itoken0 = IERC20::new(token0, provider.clone());
    let itoken1 = IERC20::new(token1, provider.clone());

    let multicall = provider
        .multicall()
        .add(itoken0.symbol())
        .add(itoken1.symbol());

    let (symbol0, symbol1) = multicall.aggregate().await?;
    write_symbol_cache(target, Some(format!("{symbol0}|{symbol1}"))).await?;

    Ok(())
}

#[cfg_attr(feature = "hotpath", hotpath::measure)]
async fn get_erc20_symbol(target: Address, provider: &Arc<GenericProvider>) -> Result<()> {
    match IERC20::new(target, provider.clone()).symbol().call().await {
        Ok(symbol) => {
            write_symbol_cache(target, Some(symbol)).await?;
        }
        Err(_e) => {
            write_symbol_cache(target, None).await?;
        }
    };

    Ok(())
}

async fn read_symbols_cache(target: Address) -> Result<CachedEntry> {
    {
        let cache = SYMBOLS_MEMORY_CACHE.read().await;
        if let Some(entry) = cache.get(&target) {
            return Ok(entry.clone());
        }
    }

    match cacache::read(&symbols_cache_dir(), target.to_string()).await {
        Ok(bytes) => {
            let name = String::from_utf8(bytes)
                .map_err(|e| eyre::eyre!("Invalid UTF-8 in cache: {}", e))?;
            let entry = if name.len() == 1 {
                CachedEntry::KnownEmpty
            } else {
                CachedEntry::Known(name)
            };

            {
                let mut cache = SYMBOLS_MEMORY_CACHE.write().await;
                cache.insert(target, entry.clone());
            }

            Ok(entry)
        }
        Err(_) => {
            {
                let mut cache = SYMBOLS_MEMORY_CACHE.write().await;
                cache.insert(target, CachedEntry::Unknown);
            }
            Ok(CachedEntry::Unknown)
        }
    }
}

pub async fn symbol_lookup_only_cached(target: Address) -> Result<Option<String>> {
    match read_symbols_cache(target).await? {
        CachedEntry::Known(name) => Ok(Some(name)),
        CachedEntry::KnownEmpty => Ok(None),
        CachedEntry::Unknown => Ok(None),
    }
}

pub async fn symbol_lookup_async(
    target: Address,
    signature_type: Option<MEVLogSignatureType>,
    symbols_lookup: &ERC20SymbolLookupWorker,
) -> Result<Option<String>> {
    let Some(signature_type) = signature_type else {
        return Ok(None);
    };

    match read_symbols_cache(target).await? {
        CachedEntry::Known(name) => Ok(Some(name)),
        CachedEntry::KnownEmpty => Ok(None),
        CachedEntry::Unknown => {
            symbols_lookup.send((target, signature_type))?;
            Ok(None)
        }
    }
}

async fn write_symbol_cache(target: Address, name: Option<String>) -> Result<()> {
    let name_record = match &name {
        Some(name) => name.as_str(),
        None => MISSING_SYMBOL,
    };

    let entry = match &name {
        Some(name) => CachedEntry::Known(name.clone()),
        None => CachedEntry::KnownEmpty,
    };

    {
        let mut cache = SYMBOLS_MEMORY_CACHE.write().await;
        cache.insert(target, entry);
    }

    match cacache::write(
        &symbols_cache_dir(),
        target.to_string(),
        name_record.as_bytes(),
    )
    .await
    {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("Error writing ENS cache: {}", e);
        }
    };
    Ok(())
}

fn symbols_cache_dir() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog/.symbols-cache")
}
