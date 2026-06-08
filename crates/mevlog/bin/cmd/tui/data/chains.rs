use eyre::Result;
use mevlog::{ChainEntryJson, cmds};
use std::time::Duration;
use tokio::time::timeout;

#[hotpath::measure(log = true, future = true)]
pub(crate) async fn fetch_chains(filter: Option<String>) -> Result<Vec<ChainEntryJson>> {
    match timeout(
        Duration::from_secs(10),
        cmds::chains::chains(filter.as_deref(), None, &[]),
    )
    .await
    {
        Ok(res) => res,
        Err(_) => eyre::bail!("chains timed out after 10 seconds"),
    }
}
