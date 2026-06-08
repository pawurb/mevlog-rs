use std::sync::Arc;

use alloy::providers::Provider;
use eyre::{Result, bail, eyre};

use crate::{
    GenericProvider,
    misc::ens_utils::{ens_addr_lookup, ensure_ens_supported},
};

/// The `query` command's `--sql` macros, each wrapped in braces. The plain-token
/// macros are constants; the `RESOLVE_ENS` token carries an argument, so its
/// grammar lives in [`extract_ens_names`] / [`ens_macro_token`].
pub const LATEST_BLOCK_MACRO: &str = "{LATEST_BLOCK()}";
pub const NATIVE_TOKEN_PRICE_MACRO: &str = "{NATIVE_TOKEN_PRICE()}";
const ENS_MACRO_OPEN: &str = "{RESOLVE_ENS(\"";
const ENS_MACRO_CLOSE: &str = "\")}";

/// Expands the macro tokens supported in `--sql` into concrete literals, fetching
/// each value only when its token is present:
/// - `{LATEST_BLOCK()}` -> the chain's current latest block number (one RPC call).
/// - `{NATIVE_TOKEN_PRICE()}` -> the native token's USD price; errors if no price
///   is available rather than silently producing wrong USD figures.
/// - `{RESOLVE_ENS("name.eth")}` -> the resolved address as a `X'..'` blob literal
///   (Ethereum mainnet only).
pub(crate) async fn substitute_sql_macros(
    sql: &str,
    provider: &Arc<GenericProvider>,
    chain_id: u64,
    native_token_price: Option<f64>,
) -> Result<String> {
    let mut out = sql.to_string();

    if out.contains(LATEST_BLOCK_MACRO) {
        let latest = provider.get_block_number().await?;
        out = out.replace(LATEST_BLOCK_MACRO, &latest.to_string());
    }

    if out.contains(NATIVE_TOKEN_PRICE_MACRO) {
        let price = native_token_price.ok_or_else(|| {
            eyre!(
                "{NATIVE_TOKEN_PRICE_MACRO} used but no price is available; pass \
                 --native-token-price or use a chain with a Chainlink oracle"
            )
        })?;
        out = out.replace(NATIVE_TOKEN_PRICE_MACRO, &price.to_string());
    }

    let ens_names = extract_ens_names(&out)?;
    if !ens_names.is_empty() {
        ensure_ens_supported(chain_id)?;
        for name in ens_names {
            let addr = ens_addr_lookup(&name, provider)
                .await?
                .ok_or_else(|| eyre!("ENS name {name:?} did not resolve to an address"))?;
            out = out.replace(
                &ens_macro_token(&name),
                &format!("X'{}'", hex::encode(addr)),
            );
        }
    }

    Ok(out)
}

/// Extracts the names from every `{RESOLVE_ENS("name.eth")}` token in `sql`,
/// deduplicated. Each name must end with `.eth`; anything else is rejected so a
/// typo doesn't silently fall through to an unresolved token.
pub(crate) fn extract_ens_names(sql: &str) -> Result<Vec<String>> {
    let mut names = Vec::new();
    let mut rest = sql;
    while let Some(start) = rest.find(ENS_MACRO_OPEN) {
        let after = &rest[start + ENS_MACRO_OPEN.len()..];
        let end = after
            .find(ENS_MACRO_CLOSE)
            .ok_or_else(|| eyre!("unterminated {{RESOLVE_ENS(\"...\")}} token in --sql"))?;
        let name = &after[..end];

        if !name.ends_with(".eth") {
            bail!("RESOLVE_ENS argument {name:?} must be an ENS name ending in .eth");
        }

        if !names.contains(&name.to_string()) {
            names.push(name.to_string());
        }
        rest = &after[end + ENS_MACRO_CLOSE.len()..];
    }

    Ok(names)
}

/// Builds the `{RESOLVE_ENS("name")}` token for `name`, so substitution uses the
/// same grammar that [`extract_ens_names`] parses.
pub(crate) fn ens_macro_token(name: &str) -> String {
    format!("{ENS_MACRO_OPEN}{name}{ENS_MACRO_CLOSE}")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn extract_ens_names_dedups_and_requires_eth_suffix() -> Result<()> {
        let names = extract_ens_names(
            "SELECT {RESOLVE_ENS(\"a.eth\")}, {RESOLVE_ENS(\"b.eth\")}, {RESOLVE_ENS(\"a.eth\")}",
        )?;
        assert_eq!(names, vec!["a.eth".to_string(), "b.eth".to_string()]);

        assert!(extract_ens_names("SELECT 1").unwrap().is_empty());
        assert!(extract_ens_names("SELECT {RESOLVE_ENS(\"vitalik\")}").is_err());
        assert!(extract_ens_names("SELECT {RESOLVE_ENS(\"oops.eth\"}").is_err());
        Ok(())
    }

    #[test]
    fn ens_macro_token_roundtrips_with_extract() -> Result<()> {
        let token = ens_macro_token("vitalik.eth");
        assert_eq!(token, "{RESOLVE_ENS(\"vitalik.eth\")}");
        assert_eq!(extract_ens_names(&token)?, vec!["vitalik.eth".to_string()]);
        Ok(())
    }
}
