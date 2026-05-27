use alloy::providers::Provider;
use eyre::{Result, eyre};

#[derive(Debug, PartialEq)]
pub struct BlocksRange {
    pub from: u64,
    pub to: u64,
}

impl BlocksRange {
    pub fn size(&self) -> u64 {
        if self.from > self.to {
            panic!("Invalid block range")
        }

        self.to - self.from + 1
    }

    pub async fn from_str(
        input: &str,
        provider: &impl Provider,
        latest_offset: Option<u64>,
    ) -> Result<Self> {
        let parts: Vec<&str> = input.split(':').collect();

        let result: Result<Self> = match parts.as_slice() {
            ["latest"] => {
                let latest_block = get_latest_block(provider, latest_offset).await?;

                Ok(BlocksRange {
                    from: latest_block,
                    to: latest_block,
                })
            }
            [single] => {
                let block = single
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid block number: '{}'", single))?;

                let latest_block = get_latest_block(provider, latest_offset).await?;
                if block > latest_block {
                    eyre::bail!(
                        "Block number '{}' exceeds latest block '{}'",
                        block,
                        latest_block
                    )
                }

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

                let latest_block = get_latest_block(provider, latest_offset).await?;
                if to > latest_block {
                    eyre::bail!("End block '{}' exceeds latest block '{}'", to, latest_block)
                }

                Ok(BlocksRange { from, to })
            }
            [from, to] if *to == "latest" || to.is_empty() => {
                let num_blocks = from
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid negative block range: '{}'", from))?;

                let latest_block = get_latest_block(provider, latest_offset).await?;
                let from = latest_block.saturating_sub(num_blocks - 1);
                let to = latest_block;

                Ok(BlocksRange { from, to })
            }

            _ => eyre::bail!("Invalid block range format: '{}'", input),
        };

        result
    }
}

async fn get_latest_block(provider: &impl Provider, latest_offset: Option<u64>) -> Result<u64> {
    let mut latest_block = provider
        .get_block_number()
        .await
        .map_err(eyre::Report::from)?;
    if let Some(offset) = latest_offset {
        latest_block = latest_block.saturating_sub(offset);
    }
    Ok(latest_block)
}
