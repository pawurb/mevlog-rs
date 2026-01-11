use std::{borrow::Cow, collections::HashMap, path::PathBuf, sync::Arc};

use alloy::{primitives::Keccak256, sol};
use eyre::{Result, bail};
use revm::primitives::{Address, B256, address};
use tokio::sync::{
    RwLock,
    mpsc::{self, UnboundedSender},
};

use super::shared_init::init_provider;
use crate::{GenericProvider, misc::symbol_utils::CachedEntry, models::evm_chain::EVMChain};

pub const ENS_REVERSE_REGISTRAR_DOMAIN: &str = "addr.reverse";

sol! {
    #[sol(rpc)]
    contract ENSLookupOracle {
        function getNameForNode(bytes32 node) public view returns (string memory);
        function getAddressForNode(bytes32 node) public view returns (address);
    }
}

const ENS_LOOKUP: Address = address!("0x80800fB4e3c77a25638aF8607f5274541831CF07");
const MISSING_NAME: &str = "N";

static ENS_MEMORY_CACHE: std::sync::LazyLock<RwLock<HashMap<Address, CachedEntry>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

#[derive(Debug)]
pub enum ENSLookup {
    Sync,
    Async(UnboundedSender<Address>),
    OnlyCached,
    Disabled,
}

#[hotpath::measure_all]
impl ENSLookup {
    pub async fn lookup_mode(
        ens_query: Option<String>,
        ens_lookup_worker: UnboundedSender<Address>,
        chain: &EVMChain,
        ens_enabled: bool,
        provider: &Arc<GenericProvider>,
    ) -> Result<ENSLookup> {
        if chain.chain_id != 1 {
            return Ok(ENSLookup::Disabled);
        }

        if let Some(ens_query) = ens_query
            && !known_ens_name(&ens_query).await
        {
            let addr = ens_addr_lookup(ens_query.clone(), provider).await?;
            let Some(addr) = addr else {
                bail!("{ens_query} is not a registered ENS name");
            };

            write_ens_cache(addr, Some(ens_query)).await?;
        };

        if !ens_enabled {
            return Ok(ENSLookup::OnlyCached);
        }

        Ok(ENSLookup::Async(ens_lookup_worker))
    }
}

pub async fn ens_lookup_async(
    target: Address,
    ens_sender: &UnboundedSender<Address>,
) -> Result<Option<String>> {
    match read_ens_cache(target).await? {
        CachedEntry::Known(name) => Ok(Some(name)),
        CachedEntry::KnownEmpty => Ok(None),
        CachedEntry::Unknown => {
            ens_sender.send(target)?;
            Ok(None)
        }
    }
}

pub async fn ens_lookup_only_cached(target: Address) -> Result<Option<String>> {
    match read_ens_cache(target).await? {
        CachedEntry::Known(name) => Ok(Some(name)),
        CachedEntry::KnownEmpty => Ok(None),
        CachedEntry::Unknown => Ok(None),
    }
}

pub async fn known_ens_name(name: &str) -> bool {
    cacache::read(&ens_cache_dir(), name).await.is_ok()
}

pub async fn ens_lookup_sync(
    target: Address,
    provider: &Arc<GenericProvider>,
) -> Result<Option<String>> {
    match read_ens_cache(target).await? {
        CachedEntry::Known(name) => Ok(Some(name)),
        CachedEntry::KnownEmpty => Ok(None),
        CachedEntry::Unknown => {
            let name = ens_name_lookup(target, provider).await?;
            write_ens_cache(target, name.clone()).await?;
            Ok(name)
        }
    }
}

async fn ens_addr_lookup(
    target: String,
    provider: &Arc<GenericProvider>,
) -> Result<Option<Address>> {
    let bytes = namehash(&target);
    let ens_lookup = ENSLookupOracle::new(ENS_LOOKUP, provider);
    let addr = ens_lookup.getAddressForNode(bytes).call().await?;

    if addr.is_zero() {
        Ok(None)
    } else {
        Ok(Some(addr))
    }
}

async fn ens_name_lookup(
    target: Address,
    provider: &Arc<GenericProvider>,
) -> Result<Option<String>> {
    let name = reverse_address(&target);
    let node = namehash(&name);

    let ens_lookup = ENSLookupOracle::new(ENS_LOOKUP, provider);
    let name = ens_lookup.getNameForNode(node).call().await?;
    Ok(if name.is_empty() { None } else { Some(name) })
}

#[hotpath::measure(log = true)]
async fn read_ens_cache(target: Address) -> Result<CachedEntry> {
    {
        let cache = ENS_MEMORY_CACHE.read().await;
        if let Some(entry) = cache.get(&target) {
            return Ok(entry.clone());
        }
    }

    match cacache::read(&ens_cache_dir(), target.to_string()).await {
        Ok(bytes) => {
            let name = String::from_utf8(bytes)
                .map_err(|e| eyre::eyre!("Invalid UTF-8 in cache: {}", e))?;
            let entry = if name.len() == 1 {
                CachedEntry::KnownEmpty
            } else {
                CachedEntry::Known(name)
            };

            {
                let mut cache = ENS_MEMORY_CACHE.write().await;
                cache.insert(target, entry.clone());
            }

            Ok(entry)
        }
        Err(_) => {
            {
                let mut cache = ENS_MEMORY_CACHE.write().await;
                cache.insert(target, CachedEntry::Unknown);
            }
            Ok(CachedEntry::Unknown)
        }
    }
}

async fn write_ens_cache(target: Address, name: Option<String>) -> Result<()> {
    if let Some(name) = &name {
        cacache::write(&ens_cache_dir(), name.to_string(), "T").await?;
    };

    let name_record = match &name {
        Some(name) => name.as_str(),
        None => MISSING_NAME,
    };

    let entry = match &name {
        Some(name) => CachedEntry::Known(name.clone()),
        None => CachedEntry::KnownEmpty,
    };

    {
        let mut cache = ENS_MEMORY_CACHE.write().await;
        cache.insert(target, entry);
    }

    match cacache::write(&ens_cache_dir(), target.to_string(), name_record.as_bytes()).await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("Error writing ENS cache: {}", e);
        }
    };
    Ok(())
}

#[allow(unused_mut)]
pub fn start_ens_lookup_worker(rpc_url: &str) -> mpsc::UnboundedSender<Address> {
    let (tx, mut rx) = hotpath::channel!(mpsc::unbounded_channel::<Address>(), log = true);

    let rpc_url = rpc_url.to_string();
    tokio::spawn(async move {
        let provider = init_provider(&rpc_url).await.unwrap();
        let provider = Arc::new(provider);

        while let Some(target) = rx.recv().await {
            let name = match ens_name_lookup(target, &provider).await {
                Ok(name) => name,
                Err(e) => {
                    tracing::error!("Error looking up ENS name: {}", e);
                    continue;
                }
            };
            match write_ens_cache(target, name.clone()).await {
                Ok(_) => (),
                Err(e) => {
                    tracing::error!("Error writing ENS cache: {}", e);
                    continue;
                }
            }
        }
    });

    tx
}

fn ens_cache_dir() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog/.ens-cache")
}

// source https://github.com/foundry-rs/foundry/blob/0a2ad0034dded199812bc9a97ea96f59f9b87354/crates/common/src/ens.rs#L168
pub fn namehash(name: &str) -> B256 {
    if name.is_empty() {
        return B256::ZERO;
    }

    // Remove the variation selector `U+FE0F` if present.
    const VARIATION_SELECTOR: char = '\u{fe0f}';
    let name = if name.contains(VARIATION_SELECTOR) {
        Cow::Owned(name.replace(VARIATION_SELECTOR, ""))
    } else {
        Cow::Borrowed(name)
    };

    // Generate the node starting from the right.
    // This buffer is `[node @ [u8; 32], label_hash @ [u8; 32]]`.
    let mut buffer = [0u8; 64];
    for label in name.rsplit('.') {
        // node = keccak256([node, keccak256(label)])

        // Hash the label.
        let mut label_hasher = Keccak256::new();
        label_hasher.update(label.as_bytes());
        label_hasher.finalize_into(&mut buffer[32..]);

        // Hash both the node and the label hash, writing into the node.
        let mut buffer_hasher = Keccak256::new();
        buffer_hasher.update(buffer.as_slice());
        buffer_hasher.finalize_into(&mut buffer[..32]);
    }
    buffer[..32].try_into().unwrap()
}

pub fn reverse_address(addr: &Address) -> String {
    format!("{addr:x}.{ENS_REVERSE_REGISTRAR_DOMAIN}")
}
