use alloy::providers::Provider;
use eyre::{eyre, Result};
use mevlog::misc::ens_utils::ENSLookup;
use mevlog::misc::shared_init::{init_deps, ConnOpts};
use mevlog::misc::utils::SEPARATORER;
use mevlog::models::mev_block::process_block;
use mevlog::models::txs_filter::{SharedFilterOpts, TxsFilter};

#[derive(Debug, clap::Parser)]
pub struct SearchArgs {
    #[arg(short = 'b', long, help_heading = "Block number or range to filter by (e.g., '22030899', 'latest', '22030800:22030900' '50:latest', '50:'", num_args(1..))]
    blocks: String,

    #[command(flatten)]
    filter: SharedFilterOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl SearchArgs {
    pub async fn run(&self) -> Result<()> {
        let shared_deps = init_deps(&self.conn_opts).await?;
        let sqlite = shared_deps.sqlite;
        let provider = shared_deps.provider;

        let mev_filter = TxsFilter::new(&self.filter, None, self.conn_opts.trace.as_ref(), false)?;

        let ens_lookup = if ENSLookup::sync_lookup(mev_filter.ens_query()).await {
            ENSLookup::Sync
        } else {
            ENSLookup::Async(shared_deps.ens_lookup_worker)
        };

        let latest_block = provider.get_block_number().await?;
        let block_range = BlocksRange::from_str(&self.blocks, latest_block)?;

        if !mev_filter.top_metadata {
            println!("{}", SEPARATORER);
        }
        for block_number in block_range.from..=block_range.to {
            process_block(
                &provider,
                &sqlite,
                block_number,
                &ens_lookup,
                &shared_deps.symbols_lookup_worker,
                &mev_filter,
                &self.conn_opts,
            )
            .await?;
        }

        if mev_filter.top_metadata {
            println!("{}", SEPARATORER);
        }
        // Allow async ENS and symbols lookups to finish
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
pub struct BlocksRange {
    from: u64,
    to: u64,
}

impl BlocksRange {
    pub fn from_str(input: &str, latest_block: u64) -> Result<Self> {
        let parts: Vec<&str> = input.split(':').collect();

        let result: Result<Self> = match parts.as_slice() {
            ["latest"] => Ok(BlocksRange {
                from: latest_block,
                to: latest_block,
            }),
            [single] => {
                let block = single
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid block number: '{}'", single))?;
                Ok(BlocksRange {
                    from: block,
                    to: block,
                })
            }
            [from, to]
                if from.chars().all(|c| c.is_numeric())
                    && to.chars().all(|c| c.is_numeric())
                    && !to.is_empty() =>
            {
                let from = from
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid start block: '{}'", from))?;
                let to = to
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid end block: '{}'", to))?;

                if from > to {
                    eyre::bail!(
                        "Start block '{}' must be less than or equal to end block '{}'",
                        from,
                        to
                    )
                }

                Ok(BlocksRange { from, to })
            }
            [from, to] if *to == "latest" || to.is_empty() => {
                let num_blocks = from
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid negative block range: '{}'", from))?;

                if num_blocks > latest_block {
                    return Err(eyre!(
                        "Invalid range: '{}' exceeds the latest block '{}'",
                        num_blocks,
                        latest_block
                    ));
                }

                let from = latest_block - num_blocks + 1;
                let to = latest_block;

                Ok(BlocksRange { from, to })
            }

            _ => eyre::bail!("Invalid block range format: '{}'", input),
        };

        let result = result?;

        if result.to > latest_block {
            eyre::bail!(
                "Invalid range: end block '{}' exceeds the latest block '{}'",
                result.to,
                latest_block
            )
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_block() {
        let latest_block = 1500;
        let range = BlocksRange::from_str("890", latest_block).unwrap();
        assert_eq!(range.from, 890);
        assert_eq!(range.to, 890);
    }

    #[test]
    fn test_numeric_block_range() {
        let latest_block = 2000;
        let range = BlocksRange::from_str("999:1200", latest_block).unwrap();
        assert_eq!(range.from, 999);
        assert_eq!(range.to, 1200);
    }

    #[test]
    fn test_negative_block_range() {
        let latest_block = 1000;
        let range = BlocksRange::from_str("100:", latest_block).unwrap();
        assert_eq!(range.from, 901); // latest_block - 99
        assert_eq!(range.to, 1000); // latest_block
    }

    #[test]
    fn test_latest_block_range() {
        let latest_block = 5000;
        let range = BlocksRange::from_str("2:latest", latest_block).unwrap();
        assert_eq!(range.from, 4999); // latest_block - 1
        assert_eq!(range.to, 5000); // latest_block
    }

    #[test]
    fn test_invalid_block_format() {
        let latest_block = 1000;
        let err = BlocksRange::from_str("abc:def", latest_block).unwrap_err();
        assert!(err.to_string().contains("Invalid block range format"));
    }

    #[test]
    fn test_invalid_start_block() {
        let latest_block = 2000;
        let err = BlocksRange::from_str("abc:1200", latest_block).unwrap_err();
        assert!(err.to_string().contains("Invalid block range format"));
    }

    #[test]
    fn test_invalid_end_block() {
        let latest_block = 2000;
        let err = BlocksRange::from_str("999:xyz", latest_block).unwrap_err();
        assert!(err.to_string().contains("Invalid block range format"));
    }

    #[test]
    fn test_range_exceeding_latest_block() {
        let latest_block = 1500;
        let err = BlocksRange::from_str("1400:1600", latest_block).unwrap_err();
        assert!(err
            .to_string()
            .contains("Invalid range: end block '1600' exceeds the latest block '1500'"));
    }

    #[test]
    fn test_start_block_greater_than_end() {
        let latest_block = 1500;
        let err = BlocksRange::from_str("1200:1100", latest_block).unwrap_err();
        assert!(err
            .to_string()
            .contains("Start block '1200' must be less than or equal to end block '1100'"));
    }
}
