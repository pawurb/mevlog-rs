use std::{borrow::Cow, sync::Arc};

use alloy::{primitives::Keccak256, sol};
use eyre::{Result, bail};
use revm::primitives::{Address, B256, address};

use crate::GenericProvider;

pub const ENS_REVERSE_REGISTRAR_DOMAIN: &str = "addr.reverse";

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
pub fn ensure_ens_supported(chain_id: u64) -> Result<()> {
    if chain_id != ENS_CHAIN_ID {
        bail!("ENS resolution is only supported on Ethereum mainnet (chain ID {ENS_CHAIN_ID})");
    }
    Ok(())
}

/// Forward resolution: ENS name -> address.
pub async fn ens_addr_lookup(
    name: &str,
    provider: &Arc<GenericProvider>,
) -> Result<Option<Address>> {
    let node = namehash(name);
    let ens_lookup = ENSLookupOracle::new(ENS_LOOKUP, provider);
    let addr = ens_lookup.getAddressForNode(node).call().await?;

    if addr.is_zero() {
        Ok(None)
    } else {
        Ok(Some(addr))
    }
}

/// Reverse resolution: address -> ENS name.
pub async fn ens_name_lookup(
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

    #[test]
    fn ensure_ens_supported_rejects_non_mainnet() {
        assert!(ensure_ens_supported(1).is_ok());
        assert!(ensure_ens_supported(8453).is_err());
    }
}
