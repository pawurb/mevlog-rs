use eyre::{bail, eyre, Result};
use regex::Regex;
use revm::primitives::{Address, U256};
use std::{
    collections::HashSet,
    fmt::{self, Display},
    str::FromStr,
};

use crate::misc::shared_init::TraceMode;

use super::mev_transaction::MEVTransaction;

#[derive(Clone, Debug, clap::Parser)]
pub struct SharedFilterOpts {
    #[arg(short = 'f', long, help = "Filter by tx source address or ENS name")]
    pub from: Option<String>,

    #[arg(short = 'p', long, help_heading = "Tx position or position range in a block (e.g., '0' or '0:10'", num_args(1..))]
    pub position: Option<String>,

    #[arg(
        short = 't',
        long,
        help = "Filter by contracts with storage changed by the transaction"
    )]
    pub touching: Option<Address>,

    #[arg(
        alias = "e",
        long,
        help = "Include txs by event names matching the provided regex or signature and optionally an address"
    )]
    pub event: Vec<String>,

    #[arg(
        alias = "ne",
        long,
        help = "Exclude txs by event names matching the provided regex or signature and optionally an address"
    )]
    pub not_event: Option<String>,
    #[arg(
        alias = "m",
        long,
        help = "Include txs by root method names matching the provided regex or signature"
    )]
    pub method: Option<String>,

    #[arg(
        alias = "tc",
        long,
        help = "Filter by tx cost in wei (e.g., 'ge10000000000000000', 'le10000000000000000')"
    )]
    pub tx_cost: Option<String>,

    #[arg(
        alias = "rtc",
        long,
        help = "Filter by real (including coinbase bribe) tx cost in wei (e.g., 'ge10000000000000000', 'le10000000000000000')"
    )]
    pub real_tx_cost: Option<String>,

    #[arg(
        alias = "gp",
        long,
        help = "Filter by effective gas price in wei (e.g., 'ge2000000000', 'le2000000000')"
    )]
    pub gas_price: Option<String>,

    #[arg(
        alias = "rgp",
        long,
        help = "Filter by real (including coinbase bribe) effective gas price in wei (e.g., 'gte2000000000', 'lte2000000000')"
    )]
    pub real_gas_price: Option<String>,

    #[arg(short, long, alias = "r", help = "Reverse the order of txs")]
    pub reverse: bool,
}

#[derive(Debug)]
pub struct GasPriceQuery {
    pub gas_price: U256,
    pub operator: DiffOperator,
}

impl GasPriceQuery {
    pub fn matches(&self, gas_price: U256) -> bool {
        match self.operator {
            DiffOperator::GreaterOrEq => gas_price >= self.gas_price,
            DiffOperator::LessOrEq => gas_price <= self.gas_price,
        }
    }
}

impl FromStr for GasPriceQuery {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (operator, gas_price) = parse_query(s)?;

        Ok(GasPriceQuery {
            operator,
            gas_price,
        })
    }
}

#[derive(Debug)]
pub struct TxCostQuery {
    pub diff: U256,
    pub operator: DiffOperator,
}

impl TxCostQuery {
    pub fn matches(&self, diff: U256) -> bool {
        match self.operator {
            DiffOperator::GreaterOrEq => diff >= self.diff,
            DiffOperator::LessOrEq => diff <= self.diff,
        }
    }
}

impl FromStr for TxCostQuery {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (operator, diff) = parse_query(s)?;

        Ok(TxCostQuery { operator, diff })
    }
}

fn parse_query(s: &str) -> Result<(DiffOperator, U256)> {
    let trimmed = s.trim();
    if trimmed.len() < 2 {
        eyre::bail!("Invalid coinbase diff query: '{}'", s);
    }

    let (op_str, num_str) = trimmed.split_at(2);
    let operator = DiffOperator::from_str(op_str).map_err(|e| eyre!("Parse error: {:?}", e))?;
    let diff = num_str.parse().map_err(|e| eyre!("Parse error: {:?}", e))?;

    Ok((operator, diff))
}

#[derive(Debug)]
pub enum DiffOperator {
    GreaterOrEq,
    LessOrEq,
}

impl FromStr for DiffOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ge" => Ok(DiffOperator::GreaterOrEq),
            "le" => Ok(DiffOperator::LessOrEq),
            _ => Err(format!(
                "Invalid operator: '{}' use 'lte' (Less or Equal) or 'ge' (Greater or Equal)",
                s
            )),
        }
    }
}

#[derive(Debug)]
pub struct TxsFilter {
    pub tx_indexes: Option<HashSet<u64>>,
    pub tx_from: Option<FromFilter>,
    pub touching: Option<Address>,
    pub tx_position: Option<PositionRange>,
    pub events: Vec<EventQuery>,
    pub not_events: Vec<EventQuery>,
    pub match_method: Option<SignatureQuery>,
    pub tx_cost: Option<TxCostQuery>,
    pub real_tx_cost: Option<TxCostQuery>,
    pub gas_price: Option<GasPriceQuery>,
    pub real_gas_price: Option<GasPriceQuery>,
    pub reversed_order: bool,
}

impl TxsFilter {
    pub fn new(
        filter_opts: &SharedFilterOpts,
        tx_indexes: Option<HashSet<u64>>,
        trace_mode: Option<&TraceMode>,
        watch_mode: bool,
    ) -> Result<Self> {
        if trace_mode.is_none() {
            if filter_opts.touching.is_some() {
                eyre::bail!(
                    "'--touching' filter is supported only with --trace [rpc|revm] enabled "
                )
            }

            if filter_opts.real_tx_cost.is_some() {
                eyre::bail!(
                    "'--real-tx-cost' filter is supported only with --trace [rpc|revm] enabled "
                )
            }

            if filter_opts.real_gas_price.is_some() {
                eyre::bail!(
                    "'--real-gas-price' filter is supported only with --trace [rpc|revm] enabled "
                )
            }
        }

        Ok(Self {
            tx_cost: match filter_opts.tx_cost {
                Some(ref query) => Some(query.parse()?),
                None => None,
            },
            gas_price: match filter_opts.gas_price {
                Some(ref query) => Some(query.parse()?),
                None => None,
            },
            real_tx_cost: match filter_opts.real_tx_cost {
                Some(ref query) => Some(query.parse()?),
                None => None,
            },
            real_gas_price: match filter_opts.real_gas_price {
                Some(ref query) => Some(query.parse()?),
                None => None,
            },
            tx_indexes,
            tx_from: FromFilter::new(filter_opts.from.as_deref())?,
            touching: filter_opts.touching,
            tx_position: match filter_opts.position {
                Some(ref position) => Some(position.parse()?),
                None => {
                    if watch_mode {
                        Some(PositionRange { from: 0, to: 4 })
                    } else {
                        None
                    }
                }
            },
            events: filter_opts
                .event
                .iter()
                .map(|query| query.parse())
                .collect::<Result<Vec<_>>>()?,
            not_events: filter_opts
                .not_event
                .iter()
                .map(|query| query.parse())
                .collect::<Result<Vec<_>>>()?,
            match_method: match filter_opts.method {
                Some(ref query) => Some(query.parse()?),
                None => None,
            },
            reversed_order: filter_opts.reverse,
        })
    }

    pub fn prefetch_receipts(&self) -> bool {
        self.tx_cost.is_some()
            || self.gas_price.is_some()
            || self.real_tx_cost.is_some()
            || self.real_gas_price.is_some()
    }

    pub fn should_exclude(&self, mev_tx: &MEVTransaction) -> bool {
        if let Some(tx_cost) = &self.tx_cost {
            if !tx_cost.matches(U256::from(mev_tx.gas_tx_cost())) {
                return true;
            }
        }
        if let Some(effective_gas_price) = &self.gas_price {
            if !effective_gas_price.matches(mev_tx.effective_gas_price()) {
                return true;
            }
        }

        if let Some(full_tx_cost) = &self.real_tx_cost {
            if !full_tx_cost.matches(mev_tx.full_tx_cost()) {
                return true;
            }
        }

        if let Some(full_effective_gas_price) = &self.real_gas_price {
            if !full_effective_gas_price.matches(mev_tx.full_effective_gas_price()) {
                return true;
            }
        }

        false
    }

    pub fn ens_filter_enabled(&self) -> bool {
        matches!(self.tx_from, Some(FromFilter::ENSName(_)))
    }
}

#[derive(Debug)]
pub struct EventQuery {
    pub signature: Option<SignatureQuery>,
    pub address: Option<Address>,
}

impl EventQuery {
    pub fn matches(&self, signature: &str, address: &Address) -> bool {
        let sig_matching = match self.signature {
            Some(ref sig_query) => sig_query.matches(signature),
            None => true,
        };

        let addr_matching = match &self.address {
            Some(expected_address) => expected_address == address,
            None => true,
        };

        sig_matching && addr_matching
    }
}

impl FromStr for EventQuery {
    type Err = eyre::Error;

    fn from_str(input: &str) -> Result<Self> {
        let parts: Vec<&str> = input.split('|').collect();

        if parts.len() == 2 {
            // Case: "regex|address"
            let signature = parts[0].parse::<SignatureQuery>()?;
            let address = Some(parts[1].parse::<Address>()?);
            Ok(EventQuery {
                signature: Some(signature),
                address,
            })
        } else if parts.len() == 1 {
            if let Ok(address) = parts[0].parse::<Address>() {
                return Ok(EventQuery {
                    signature: None,
                    address: Some(address),
                });
            }

            if let Ok(signature) = parts[0].parse::<SignatureQuery>() {
                return Ok(EventQuery {
                    signature: Some(signature),
                    address: None,
                });
            }

            bail!("Invalid input: Must be either 'query|address' or a valid address")
        } else {
            bail!("Invalid input format")
        }
    }
}

#[derive(Debug)]
pub enum SignatureQuery {
    Name(String),
    Regex(Regex),
}

impl SignatureQuery {
    pub fn matches(&self, signature: &str) -> bool {
        match self {
            SignatureQuery::Name(name) => name == signature,
            SignatureQuery::Regex(regex) => regex.is_match(signature),
        }
    }
}

impl Display for SignatureQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignatureQuery::Name(name) => write!(f, "{}", name),
            SignatureQuery::Regex(regex) => write!(f, "/{}/", regex),
        }
    }
}

impl FromStr for SignatureQuery {
    type Err = eyre::Error;

    fn from_str(input: &str) -> Result<Self> {
        if input.starts_with('/') && input.ends_with('/') {
            let regex = Regex::new(&input[1..input.len() - 1])?;
            Ok(SignatureQuery::Regex(regex))
        } else {
            Ok(SignatureQuery::Name(input.to_string()))
        }
    }
}

#[derive(Debug)]
pub enum FromFilter {
    Address(Address),
    ENSName(String),
}

impl FromFilter {
    pub fn new(value: Option<&str>) -> Result<Option<Self>> {
        if value.is_none() {
            return Ok(None);
        }

        let value = value.unwrap();

        if let Ok(address) = value.parse::<Address>() {
            return Ok(Some(FromFilter::Address(address)));
        }

        if value.ends_with(".eth") {
            return Ok(Some(FromFilter::ENSName(value.to_string().to_lowercase())));
        }

        eyre::bail!(
            "Invalid input: '{}' is not an Ethereum address or ENS name",
            value
        );
    }
}

#[derive(Debug, PartialEq)]
pub struct PositionRange {
    pub from: u64,
    pub to: u64,
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
    use super::*; // Import the function from the parent module
    use std::str::FromStr;

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
        let input = &format!("Transfer(address,uint256)|{}", PEPE);
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
