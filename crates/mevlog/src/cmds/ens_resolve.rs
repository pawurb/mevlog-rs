use eyre::{Result, bail};
use serde::Serialize;

use crate::misc::{
    ens_utils::{ens_addr_lookup, ensure_ens_supported},
    shared_init::{ConnOpts, resolve_conn},
};

#[derive(Serialize)]
pub struct EnsResolveJson {
    pub name: String,
    pub address: String,
}

/// Resolves an ENS name to an address (Ethereum mainnet only).
pub async fn ens_resolve(name: &str, conn_opts: &ConnOpts) -> Result<EnsResolveJson> {
    let resolved = resolve_conn(conn_opts).await?;
    ensure_ens_supported(resolved.chain_id)?;

    let Some(address) = ens_addr_lookup(name, &resolved.provider).await? else {
        bail!("{} is not a registered ENS name", name);
    };

    Ok(EnsResolveJson {
        name: name.to_string(),
        address: format!("{address:#x}"),
    })
}
