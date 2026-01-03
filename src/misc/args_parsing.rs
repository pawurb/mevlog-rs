use std::str::FromStr;

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
                    eyre::bail!(
                        "End block '{}' exceeds latest block '{}'",
                        to,
                        latest_block
                    )
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

#[derive(Debug, PartialEq)]
pub struct PositionRange {
    pub from: u64,
    pub to: u64,
}

impl PositionRange {
    pub fn size(&self) -> u64 {
        if self.from > self.to {
            panic!("Invalid position range")
        }

        self.to - self.from + 1
    }
}

impl FromStr for PositionRange {
    type Err = eyre::Error;

    fn from_str(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.split(':').collect();

        let result: Result<Self> = match parts.as_slice() {
            // Case 1: Single position (e.g., "890")
            [single] => {
                let position = single
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid position: '{}'", single))?;
                Ok(Self {
                    from: position,
                    to: position,
                })
            }
            // Case 2: Range format (e.g., "999:1200")
            [from, to]
                if from.chars().all(|c| c.is_numeric())
                    && to.chars().all(|c| c.is_numeric())
                    && !to.is_empty() =>
            {
                let from = from
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid start position: '{}'", from))?;
                let to = to
                    .parse::<u64>()
                    .map_err(|_| eyre!("Invalid end position: '{}'", to))?;

                if from > to {
                    eyre::bail!(
                        "Start position '{}' must be less than or equal to end position '{}'",
                        from,
                        to
                    )
                }

                Ok(Self { from, to })
            }

            _ => eyre::bail!("Invalid tx position format: '{}'", input),
        };

        let result = result?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use revm::primitives::Address;

    use super::*;
    use crate::models::txs_filter::{EventQuery, SignatureQuery};

    pub const PEPE: &str = "0x6982508145454ce325ddbe47a25d4ec3d2311933";

    #[test]
    fn test_single_position() {
        let range = PositionRange::from_str("890").unwrap();
        assert_eq!(range.from, 890);
        assert_eq!(range.to, 890);
    }

    #[test]
    fn test_numeric_position_range() {
        let range = PositionRange::from_str("999:1200").unwrap();
        assert_eq!(range.from, 999);
        assert_eq!(range.to, 1200);
    }

    #[test]
    fn test_valid_signature_and_address() {
        let input = &format!("Transfer(address,uint256)|{PEPE}");
        let query = EventQuery::from_str(input).unwrap();

        assert_eq!(
            query.signature.unwrap().to_string(),
            "Transfer(address,uint256)"
        );
        assert_eq!(query.address.unwrap(), PEPE.parse::<Address>().unwrap());
    }

    #[test]
    fn test_valid_address_only() {
        let query = EventQuery::from_str(PEPE).unwrap();

        assert_eq!(query.address.unwrap(), PEPE.parse::<Address>().unwrap());
        assert!(query.signature.is_none());
    }

    #[test]
    fn test_valid_signature_only() {
        let input = "Transfer(address,uint256)";
        let query = EventQuery::from_str(input).unwrap();

        assert_eq!(
            query.signature.unwrap().to_string(),
            "Transfer(address,uint256)"
        );
        assert!(query.address.is_none());
    }

    #[test]
    fn test_invalid_address_with_signature() {
        let input = "Transfer(address,uint256)|0x123";
        let result = EventQuery::from_str(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_regexp() {
        let input = "/Transfer/";
        let query = SignatureQuery::from_str(input).unwrap();

        match query {
            SignatureQuery::Regex(regex) => assert_eq!(regex.as_str(), "Transfer"),
            _ => panic!("Expected regex"),
        }
    }
}
