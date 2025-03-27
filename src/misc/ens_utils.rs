use std::{borrow::Cow, path::PathBuf, sync::Arc};

use alloy::{primitives::Keccak256, sol};
use eyre::Result;
use revm::primitives::{address, Address, B256};
use tokio::sync::mpsc::{self, UnboundedSender};

use super::shared_init::{init_provider, ConnOpts, EVMChain, EVMChainType};
use crate::GenericProvider;

pub const ENS_REVERSE_REGISTRAR_DOMAIN: &str = "addr.reverse";

sol! {
    #[sol(rpc)]
    contract ENSLookupOracle {
        function getNameForNode(bytes32 node) public view returns (string memory);
    }
}

const ENS_LOOKUP: Address = address!("0xc69c0eb9ec6e71e97c1ed25212d203ad5010d8b2");
const MISSING_NAME: &str = "N";

#[derive(Debug)]
pub enum ENSLookup {
    Sync,
    Async(UnboundedSender<Address>),
    Disabled,
}

impl ENSLookup {
    pub async fn lookup_mode(
        ens_query: Option<String>,
        ens_lookup_worker: UnboundedSender<Address>,
        chain: &EVMChain,
    ) -> ENSLookup {
        if chain.chain_type != EVMChainType::Mainnet {
            return ENSLookup::Disabled;
        }

        if ens_query.is_none() {
            return ENSLookup::Async(ens_lookup_worker);
        }

        if known_ens_name(&ens_query.unwrap()).await {
            ENSLookup::Async(ens_lookup_worker)
        } else {
            ENSLookup::Sync
        }
    }
}

enum ENSEntry {
    Known(String),
    KnownEmpty,
    Unknown,
}

pub async fn ens_reverse_lookup_cached_async(
    target: Address,
    ens_sender: &UnboundedSender<Address>,
) -> Result<Option<String>> {
    match read_ens_cache(target).await? {
        ENSEntry::Known(name) => Ok(Some(name)),
        ENSEntry::KnownEmpty => Ok(None),
        ENSEntry::Unknown => {
            ens_sender.send(target)?;
            Ok(None)
        }
    }
}

pub async fn known_ens_name(name: &str) -> bool {
    cacache::read(&ens_cache_dir(), name).await.is_ok()
}

pub async fn ens_reverse_lookup_cached_sync(
    target: Address,
    provider: &Arc<GenericProvider>,
) -> Result<Option<String>> {
    match read_ens_cache(target).await? {
        ENSEntry::Known(name) => Ok(Some(name)),
        ENSEntry::KnownEmpty => Ok(None),
        ENSEntry::Unknown => {
            let name = ens_reverse_lookup(target, provider).await?;
            write_ens_cache(target, name.clone()).await?;
            Ok(name)
        }
    }
}

async fn ens_reverse_lookup(
    target: Address,
    provider: &Arc<GenericProvider>,
) -> Result<Option<String>> {
    let name = reverse_address(&target);
    let node = namehash(&name);

    let ens_lookup = ENSLookupOracle::new(ENS_LOOKUP, provider);
    let result = ens_lookup.getNameForNode(node).call().await?._0;
    let name = {
        let name = result;
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    };
    Ok(name)
}

async fn read_ens_cache(target: Address) -> Result<ENSEntry> {
    match cacache::read(&ens_cache_dir(), target.to_string()).await {
        Ok(bytes) => {
            let name = String::from_utf8(bytes)
                .map_err(|e| eyre::eyre!("Invalid UTF-8 in cache: {}", e))?;
            if name.len() == 1 {
                Ok(ENSEntry::KnownEmpty)
            } else {
                Ok(ENSEntry::Known(name))
            }
        }
        Err(_) => Ok(ENSEntry::Unknown),
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

    match cacache::write(&ens_cache_dir(), target.to_string(), name_record.as_bytes()).await {
        Ok(_) => (),
        Err(e) => {
            tracing::error!("Error writing ENS cache: {}", e);
        }
    };
    Ok(())
}

pub fn start_ens_lookup_worker(conn_opts: &ConnOpts) -> mpsc::UnboundedSender<Address> {
    let (tx, mut rx) = mpsc::unbounded_channel::<Address>();

    let conn_opts = conn_opts.clone();
    tokio::spawn(async move {
        let provider = init_provider(&conn_opts).await.unwrap();
        let provider = Arc::new(provider);

        while let Some(target) = rx.recv().await {
            let name = match ens_reverse_lookup(target, &provider).await {
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
