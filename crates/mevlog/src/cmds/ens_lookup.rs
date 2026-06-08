use eyre::{Result, bail};
use revm::primitives::Address;
use serde::Serialize;

use crate::misc::{
    ens_utils::{ens_name_lookup, ensure_ens_supported},
    shared_init::{ConnOpts, resolve_conn},
};

#[derive(Serialize)]
pub struct EnsLookupJson {
    pub address: String,
    pub name: String,
}

/// Reverse-resolves an address to its ENS name (Ethereum mainnet only).
pub async fn ens_lookup(address: Address, conn_opts: &ConnOpts) -> Result<EnsLookupJson> {
    let resolved = resolve_conn(conn_opts).await?;
    ensure_ens_supported(resolved.chain_id)?;

    let Some(name) = ens_name_lookup(address, &resolved.provider).await? else {
        bail!("No ENS name set for {:#x}", address);
    };

    Ok(EnsLookupJson {
        address: format!("{address:#x}"),
        name,
    })
}
