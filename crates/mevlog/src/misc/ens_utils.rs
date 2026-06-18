use std::{
    borrow::Cow,
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, LazyLock},
};

use alloy::{primitives::Keccak256, sol};
use eyre::{Result, bail};
use revm::primitives::{Address, B256, address};
use tokio::sync::RwLock;

use crate::GenericProvider;

pub const ENS_REVERSE_REGISTRAR_DOMAIN: &str = "addr.reverse";

/// In-memory cache of forward ENS resolutions (name -> address), backed by the
/// on-disk cache under `~/.mevlog/.ens-cache` so repeated lookups skip the RPC
/// `eth_call` the resolver would otherwise need.
static ENS_ADDR_CACHE: LazyLock<RwLock<HashMap<String, Address>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

fn ens_cache_dir() -> PathBuf {
    home::home_dir().unwrap().join(".mevlog").join(".ens-cache")
}

/// Reads a cached forward resolution: in-memory first, then the on-disk cache
/// (whose 20-byte address records are promoted into memory on a hit).
async fn read_ens_addr_cache(name: &str) -> Option<Address> {
    let key = name.to_lowercase();
    {
        let cache = ENS_ADDR_CACHE.read().await;
        if let Some(addr) = cache.get(&key) {
            return Some(*addr);
        }
    }

    let bytes = cacache::read(&ens_cache_dir(), &key).await.ok()?;
    let addr = Address::try_from(bytes.as_slice()).ok()?;
    ENS_ADDR_CACHE.write().await.insert(key, addr);
    Some(addr)
}

/// Persists a forward resolution to both the in-memory and on-disk caches.
async fn write_ens_addr_cache(name: &str, addr: Address) {
    let key = name.to_lowercase();
    ENS_ADDR_CACHE.write().await.insert(key.clone(), addr);
    if let Err(e) = cacache::write(&ens_cache_dir(), &key, addr.as_slice()).await {
        tracing::error!("Error writing ENS cache: {}", e);
    }
}

sol! {
    #[sol(rpc)]
    contract ENSLookupOracle {
        function getNameForNode(bytes32 node) public view returns (string memory);
        function getAddressForNode(bytes32 node) public view returns (address);
    }
}

const ENS_LOOKUP: Address = address!("0x80800fB4e3c77a25638aF8607f5274541831CF07");

// The ENS lookup oracle is only deployed on Ethereum mainnet.
const ENS_CHAIN_ID: u64 = 1;

/// Returns an error if ENS resolution is not available on the given chain.
pub(crate) fn ensure_ens_supported(chain_id: u64) -> Result<()> {
    if chain_id != ENS_CHAIN_ID {
        bail!("ENS resolution is only supported on Ethereum mainnet (chain ID {ENS_CHAIN_ID})");
    }
    Ok(())
}

/// Forward resolution: ENS name -> address.
pub(crate) async fn ens_addr_lookup(
    name: &str,
    provider: &Arc<GenericProvider>,
) -> Result<Option<Address>> {
    if let Some(addr) = read_ens_addr_cache(name).await {
        return Ok(Some(addr));
    }

    let node = namehash(name);
    let ens_lookup = ENSLookupOracle::new(ENS_LOOKUP, provider);
    let addr = ens_lookup.getAddressForNode(node).call().await?;

    if addr.is_zero() {
        Ok(None)
    } else {
        write_ens_addr_cache(name, addr).await;
        Ok(Some(addr))
    }
}

/// Reverse resolution: address -> ENS name.
pub(crate) async fn ens_name_lookup(
    target: Address,
    provider: &Arc<GenericProvider>,
) -> Result<Option<String>> {
    let name = reverse_address(&target);
    let node = namehash(&name);

    let ens_lookup = ENSLookupOracle::new(ENS_LOOKUP, provider);
    let name = ens_lookup.getNameForNode(node).call().await?;
    Ok(if name.is_empty() { None } else { Some(name) })
}

// source https://github.com/foundry-rs/foundry/blob/0a2ad0034dded199812bc9a97ea96f59f9b87354/crates/common/src/ens.rs#L168
pub(crate) fn namehash(name: &str) -> B256 {
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

pub(crate) fn reverse_address(addr: &Address) -> String {
    format!("{addr:x}.{ENS_REVERSE_REGISTRAR_DOMAIN}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namehash_empty_is_zero() {
        assert_eq!(namehash(""), B256::ZERO);
    }

    #[test]
    fn namehash_known_vectors() {
        // Canonical ENS namehash values.
        assert_eq!(
            namehash("eth"),
            "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
                .parse::<B256>()
                .unwrap()
        );
    }

    #[test]
    fn reverse_address_format() {
        let addr = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
        assert_eq!(
            reverse_address(&addr),
            "d8da6bf26964af9d7eed9e03e53415d37aa96045.addr.reverse"
        );
    }

    #[tokio::test]
    async fn addr_cache_roundtrips_case_insensitively() {
        let addr = address!("0xd8da6bf26964af9d7eed9e03e53415d37aa96045");
        // Use a name unlikely to collide with the shared on-disk cache.
        write_ens_addr_cache("Mevlog-Test-Roundtrip.eth", addr).await;
        assert_eq!(
            read_ens_addr_cache("mevlog-test-roundtrip.eth").await,
            Some(addr)
        );
        assert_eq!(read_ens_addr_cache("never-cached-xyz.eth").await, None);
    }

    #[test]
    fn ensure_ens_supported_rejects_non_mainnet() {
        assert!(ensure_ens_supported(1).is_ok());
        assert!(ensure_ens_supported(8453).is_err());
    }
}
