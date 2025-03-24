use eyre::{bail, eyre, Result};
use regex::Regex;
use revm::primitives::{Address, U256};
use std::{
    collections::HashSet,
    fmt::{self, Display},
    str::FromStr,
};

use crate::misc::{
    args_parsing::PositionRange, eth_unit_parser::parse_eth_value, shared_init::TraceMode,
};

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
        help = "Filter by tx cost (e.g., 'ge10000000000000000', 'le0.01ether')"
    )]
    pub tx_cost: Option<String>,

    #[arg(
        alias = "rtc",
        long,
        help = "Filter by real (including coinbase bribe) tx cost (e.g., 'ge10000000000000000', 'le0.01ether')"
    )]
    pub real_tx_cost: Option<String>,

    #[arg(
        alias = "gp",
        long,
        help = "Filter by effective gas price (e.g., 'ge2000000000', 'le5gwei')"
    )]
    pub gas_price: Option<String>,

    #[arg(
        alias = "rgp",
        long,
        help = "Filter by real (including coinbase bribe) effective gas price (e.g., 'ge2000000000', 'le5gwei')"
    )]
    pub real_gas_price: Option<String>,

    #[arg(short, long, alias = "r", help = "Reverse the order of txs")]
    pub reverse: bool,

    #[arg(
        long,
        alias = "tm",
        help = "Display block and txs metadata info on top"
    )]
    pub top_metadata: bool,
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

#[allow(clippy::result_large_err)]
fn parse_query(s: &str) -> Result<(DiffOperator, U256)> {
    let trimmed = s.trim();
    if trimmed.len() < 3 {
        // Need at least "ge1"
        eyre::bail!("Invalid value query: '{}'", s);
    }

    // Extract the operator part (first 2 chars)
    let op_str = &trimmed[0..2];
    let value_str = &trimmed[2..];

    // Reuse the existing DiffOperator::from_str implementation
    let operator = DiffOperator::from_str(op_str).map_err(|e| eyre!("Parse error: {}", e))?;

    // Parse the value part with Ethereum unit support
    let value = parse_eth_value(value_str)?;

    Ok((operator, value))
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
                "Invalid operator: '{}' use 'le' (Less or Equal) or 'ge' (Greater or Equal)",
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
    pub top_metadata: bool,
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
            top_metadata: filter_opts.top_metadata,
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

    pub fn ens_query(&self) -> Option<String> {
        self.tx_from.as_ref().and_then(|from| match from {
            FromFilter::ENSName(name) => Some(name.clone()),
            _ => None,
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query() {
        // Test with wei values
        let (op, value) = parse_query("ge1000000000").unwrap();
        assert!(
            matches!(op, DiffOperator::GreaterOrEq),
            "Should be GreaterOrEq operator"
        );
        assert_eq!(
            value,
            U256::from(1000000000),
            "Should parse raw wei value correctly"
        );

        // Test with gwei values
        let (op, value) = parse_query("ge5gwei").unwrap();
        assert!(
            matches!(op, DiffOperator::GreaterOrEq),
            "Should be GreaterOrEq operator"
        );
        assert_eq!(
            value,
            U256::from(5_000_000_000_u128),
            "Should convert 5 gwei to wei correctly"
        );

        // Test with ether values
        let (op, value) = parse_query("le0.01ether").unwrap();
        assert!(
            matches!(op, DiffOperator::LessOrEq),
            "Should be LessOrEq operator"
        );
        assert_eq!(
            value,
            U256::from(10).pow(U256::from(16)),
            "Should convert 0.01 ether to wei correctly"
        );

        // Test invalid operator
        let result = parse_query("xx5gwei");
        assert!(result.is_err(), "Should reject invalid operators");
    }

    #[test]
    fn test_tx_cost_query_from_str() {
        // Test with gwei
        let query = TxCostQuery::from_str("ge5gwei").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::GreaterOrEq),
            "Should parse GreaterOrEq operator"
        );
        assert_eq!(
            query.diff,
            U256::from(5_000_000_000_u128),
            "Should parse 5 gwei correctly"
        );

        // Test with ether
        let query = TxCostQuery::from_str("le0.1ether").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::LessOrEq),
            "Should parse LessOrEq operator"
        );
        assert_eq!(
            query.diff,
            U256::from(10).pow(U256::from(17)),
            "Should parse 0.1 ether correctly"
        );

        // Test with raw wei
        let query = TxCostQuery::from_str("ge1000000").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::GreaterOrEq),
            "Should parse GreaterOrEq operator"
        );
        assert_eq!(
            query.diff,
            U256::from(1000000),
            "Should parse raw wei value correctly"
        );
    }

    #[test]
    fn test_gas_price_query_from_str() {
        // Test with gwei
        let query = GasPriceQuery::from_str("ge10gwei").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::GreaterOrEq),
            "Should parse GreaterOrEq operator"
        );
        assert_eq!(
            query.gas_price,
            U256::from(10_000_000_000_u128),
            "Should parse 10 gwei correctly"
        );

        // Test with ether (unusual but should work)
        let query = GasPriceQuery::from_str("le0.000001ether").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::LessOrEq),
            "Should parse LessOrEq operator"
        );
        assert_eq!(
            query.gas_price,
            U256::from(10).pow(U256::from(12)),
            "Should parse 0.000001 ether correctly"
        );
    }

    #[test]
    fn test_matches_functionality() {
        let tx_cost = TxCostQuery {
            diff: U256::from(5_000_000_000_u128), // 5 gwei
            operator: DiffOperator::GreaterOrEq,
        };

        // Test greater than
        assert!(
            tx_cost.matches(U256::from(6_000_000_000_u128)),
            "Should match when value is greater than threshold"
        );

        // Test equal
        assert!(
            tx_cost.matches(U256::from(5_000_000_000_u128)),
            "Should match when value is equal to threshold"
        );

        // Test less than
        assert!(
            !tx_cost.matches(U256::from(4_000_000_000_u128)),
            "Should not match when value is less than threshold"
        );

        // Now test LessOrEq
        let gas_price = GasPriceQuery {
            gas_price: U256::from(10_000_000_000_u128), // 10 gwei
            operator: DiffOperator::LessOrEq,
        };

        // Test greater than
        assert!(
            !gas_price.matches(U256::from(11_000_000_000_u128)),
            "Should not match when value is greater than threshold"
        );

        // Test equal
        assert!(
            gas_price.matches(U256::from(10_000_000_000_u128)),
            "Should match when value is equal to threshold"
        );

        // Test less than
        assert!(
            gas_price.matches(U256::from(9_000_000_000_u128)),
            "Should match when value is less than threshold"
        );
    }
}
