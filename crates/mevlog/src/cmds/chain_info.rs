use eyre::{Result, bail};

use crate::{
    ChainInfoJson, ChainInfoNoRpcsJson, RpcUrlInfo,
    misc::rpc_urls::{get_chain_id_from_rpc, get_chain_info, get_chain_info_no_benchmark},
};

/// Chain info result: with benchmarked RPC URLs, or without when `--skip-rpcs`.
pub enum ChainInfoOutput {
    Full(ChainInfoJson),
    NoRpcs(ChainInfoNoRpcsJson),
}

/// Resolves chain metadata (and, unless `skip_rpcs`, benchmarked RPC URLs) from
/// a chain ID or an RPC URL.
pub async fn chain_info(
    chain_id: Option<u64>,
    rpc_url: Option<&str>,
    skip_rpcs: bool,
    rpc_timeout_ms: u64,
    rpcs_limit: usize,
) -> Result<ChainInfoOutput> {
    let chain_id = match (chain_id, rpc_url) {
        (Some(id), _) => id,
        (None, Some(url)) => get_chain_id_from_rpc(url).await?,
        (None, None) => bail!("Either --chain-id or --rpc-url must be specified"),
    };

    if skip_rpcs {
        let chain_info_raw = get_chain_info_no_benchmark(chain_id).await?;
        Ok(ChainInfoOutput::NoRpcs(ChainInfoNoRpcsJson {
            chain_id,
            name: chain_info_raw.name.clone(),
            currency: chain_info_raw.native_currency.symbol.clone(),
            explorer_url: chain_info_raw.explorers.first().map(|e| e.url.clone()),
            native_token_price: None,
        }))
    } else {
        let chain_info_raw = get_chain_info(chain_id, rpc_timeout_ms, rpcs_limit).await?;
        if chain_info_raw.benchmarked_rpc_urls.is_empty() {
            return Err(eyre::eyre!(
                "No working RPC URLs found for chain ID {}",
                chain_id
            ));
        }

        let rpc_urls = chain_info_raw
            .benchmarked_rpc_urls
            .iter()
            .map(|(url, response_time)| RpcUrlInfo {
                url: url.clone(),
                response_time_ms: *response_time,
            })
            .collect();

        Ok(ChainInfoOutput::Full(ChainInfoJson {
            chain_id,
            name: chain_info_raw.name.clone(),
            currency: chain_info_raw.native_currency.symbol.clone(),
            explorer_url: chain_info_raw.explorers.first().map(|e| e.url.clone()),
            rpc_timeout_ms,
            rpc_urls,
        }))
    }
}
