use std::{
    collections::HashSet,
    fmt::{self, Display},
    str::FromStr,
};

use eyre::{bail, eyre, Result};
use regex::Regex;
use revm::primitives::{Address, U256};

use super::mev_transaction::MEVTransaction;
use crate::misc::{
    args_parsing::PositionRange, eth_unit_parser::parse_eth_value, shared_init::TraceMode,
};

#[derive(Clone, Debug, clap::Parser)]
pub struct SharedFilterOpts {
    #[arg(short = 'f', long, help = "Filter by tx source address or ENS name")]
    pub from: Option<String>,

    #[arg(
        long,
        help = "Filter by tx target address, ENS name, or CREATE transactions"
    )]
    pub to: Option<String>,

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
        help = "Include txs by root method names matching the provided regex, signature or signature hash"
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

    #[arg(
        long,
        help = "Filter by transaction value (e.g., 'ge1ether', 'le0.1ether')"
    )]
    pub value: Option<String>,

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
pub struct PriceQuery {
    pub gas_price: U256,
    pub operator: DiffOperator,
}

impl PriceQuery {
    pub fn matches(&self, gas_price: U256) -> bool {
        match self.operator {
            DiffOperator::GreaterOrEq => gas_price >= self.gas_price,
            DiffOperator::LessOrEq => gas_price <= self.gas_price,
        }
    }
}

impl FromStr for PriceQuery {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (operator, gas_price) = parse_price_query(s)?;

        Ok(PriceQuery {
            operator,
            gas_price,
        })
    }
}

fn parse_price_query(s: &str) -> Result<(DiffOperator, U256)> {
    let trimmed = s.trim();
    if trimmed.len() < 3 {
        // Need at least "ge1"
        eyre::bail!("Invalid value query: '{}'", s);
    }

    // Extract the operator part (first 2 chars)
    let op_str = &trimmed[0..2];
    let value_str = &trimmed[2..];

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
                "Invalid operator: '{s}' use 'le' (Less or Equal) or 'ge' (Greater or Equal)"
            )),
        }
    }
}

#[derive(Debug)]
pub struct TxsFilter {
    pub tx_indexes: Option<HashSet<u64>>,
    pub tx_from: Option<AddressFilter>,
    pub tx_to: Option<AddressFilter>,
    pub touching: Option<Address>,
    pub tx_position: Option<PositionRange>,
    pub events: Vec<EventQuery>,
    pub not_events: Vec<EventQuery>,
    pub match_method: Option<SignatureQuery>,
    pub tx_cost: Option<PriceQuery>,
    pub real_tx_cost: Option<PriceQuery>,
    pub gas_price: Option<PriceQuery>,
    pub real_gas_price: Option<PriceQuery>,
    pub value: Option<PriceQuery>,
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
            value: match filter_opts.value {
                Some(ref query) => Some(query.parse()?),
                None => None,
            },
            tx_indexes,
            tx_from: AddressFilter::new(filter_opts.from.as_deref())?,
            tx_to: AddressFilter::new(filter_opts.to.as_deref())?,
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
            AddressFilter::ENSName(name) => Some(name.clone()),
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
    NameOrHash(String),
    Regex(Regex),
}

impl SignatureQuery {
    pub fn matches(&self, signature: &str) -> bool {
        match self {
            SignatureQuery::NameOrHash(name) => name == signature,
            SignatureQuery::Regex(regex) => regex.is_match(signature),
        }
    }
}

impl Display for SignatureQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignatureQuery::NameOrHash(name) => write!(f, "{name}"),
            SignatureQuery::Regex(regex) => write!(f, "/{regex}/"),
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
            Ok(SignatureQuery::NameOrHash(input.to_string()))
        }
    }
}

#[derive(Debug)]
pub enum AddressFilter {
    Address(Address),
    ENSName(String),
    CreateCall,
}

impl AddressFilter {
    pub fn new(value: Option<&str>) -> Result<Option<Self>> {
        if value.is_none() {
            return Ok(None);
        }

        let value = value.unwrap();

        if value == "CREATE" {
            return Ok(Some(AddressFilter::CreateCall));
        }

        if let Ok(address) = value.parse::<Address>() {
            return Ok(Some(AddressFilter::Address(address)));
        }

        if value.ends_with(".eth") {
            return Ok(Some(AddressFilter::ENSName(
                value.to_string().to_lowercase(),
            )));
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
    use crate::misc::utils::GWEI_U128;

    #[test]
    fn test_gas_price_query_from_str() {
        let query = PriceQuery::from_str("ge1000000000").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::GreaterOrEq),
            "Should be GreaterOrEq operator"
        );

        assert_eq!(
            query.gas_price,
            U256::from(1000000000),
            "Should parse raw wei value correctly"
        );

        // Test with gwei values
        let query = PriceQuery::from_str("ge5gwei").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::GreaterOrEq),
            "Should be GreaterOrEq operator"
        );

        assert_eq!(
            query.gas_price,
            U256::from(GWEI_U128 * 5),
            "Should convert 5 gwei to wei correctly"
        );

        // Test with ether values
        let query = PriceQuery::from_str("le0.01ether").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::LessOrEq),
            "Should be LessOrEq operator"
        );

        // Test invalid operator
        let result = PriceQuery::from_str("xx5gwei");
        assert!(result.is_err(), "Should reject invalid operators");

        // Test with gwei
        let query = PriceQuery::from_str("ge10gwei").unwrap();
        assert!(
            matches!(query.operator, DiffOperator::GreaterOrEq),
            "Should parse GreaterOrEq operator"
        );
        assert_eq!(
            query.gas_price,
            U256::from(GWEI_U128 * 10),
            "Should parse 10 gwei correctly"
        );

        // Test with ether (unusual but should work)
        let query = PriceQuery::from_str("le0.000001ether").unwrap();
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
        let tx_cost = PriceQuery {
            gas_price: U256::from(GWEI_U128 * 5),
            operator: DiffOperator::GreaterOrEq,
        };

        // Test greater than
        assert!(
            tx_cost.matches(U256::from(GWEI_U128 * 6)),
            "Should match when value is greater than threshold"
        );

        // Test equal
        assert!(
            tx_cost.matches(U256::from(GWEI_U128 * 5)),
            "Should match when value is equal to threshold"
        );

        // Test less than
        assert!(
            !tx_cost.matches(U256::from(GWEI_U128 * 4)),
            "Should not match when value is less than threshold"
        );

        // Now test LessOrEq
        let gas_price = PriceQuery {
            gas_price: U256::from(GWEI_U128 * 10),
            operator: DiffOperator::LessOrEq,
        };

        // Test greater than
        assert!(
            !gas_price.matches(U256::from(GWEI_U128 * 11)),
            "Should not match when value is greater than threshold"
        );

        // Test equal
        assert!(
            gas_price.matches(U256::from(GWEI_U128 * 10)),
            "Should match when value is equal to threshold"
        );

        // Test less than
        assert!(
            gas_price.matches(U256::from(GWEI_U128 * 9)),
            "Should match when value is less than threshold"
        );
    }
}
