use eyre::Result;

use crate::{ChainEntryJson, misc::rpc_urls::get_all_chains};

/// Returns the ChainList chains, filtered by name substring and/or chain IDs,
/// sorted by chain ID and optionally truncated.
pub async fn chains(
    filter: Option<&str>,
    limit: Option<usize>,
    chain_ids: &[u64],
) -> Result<Vec<ChainEntryJson>> {
    let mut filtered_chains = get_all_chains().await?;

    if let Some(filter) = filter {
        let filter_lower = filter.to_lowercase();
        filtered_chains.retain(|chain| {
            chain.name.to_lowercase().contains(&filter_lower)
                || chain.chain.to_lowercase().contains(&filter_lower)
        });
    }

    if !chain_ids.is_empty() {
        filtered_chains.retain(|chain| chain_ids.contains(&chain.chain_id));
    }

    filtered_chains.sort_by_key(|chain| chain.chain_id);

    if let Some(limit) = limit {
        filtered_chains.truncate(limit);
    }

    Ok(filtered_chains
        .iter()
        .map(|chain| ChainEntryJson {
            chain_id: chain.chain_id,
            name: chain.name.clone(),
            chain: chain.chain.clone(),
            explorer_url: chain.explorers.first().map(|e| e.url.clone()),
        })
        .collect())
}
