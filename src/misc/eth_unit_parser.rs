use eyre::{eyre, Result};
use revm::primitives::U256;
use std::str::FromStr;

#[derive(Debug, Clone, Copy)]
pub enum EthUnit {
    Wei,
    Gwei,
    Ether,
}

impl FromStr for EthUnit {
    type Err = eyre::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "wei" => Ok(EthUnit::Wei),
            "gwei" => Ok(EthUnit::Gwei),
            "ether" | "eth" => Ok(EthUnit::Ether),
            _ => Err(eyre!("Unknown unit: {}", s)),
        }
    }
}

impl EthUnit {
    pub fn multiplier(&self) -> U256 {
        match self {
            EthUnit::Wei => U256::from(1),
            EthUnit::Gwei => U256::from(10).pow(U256::from(9)),
            EthUnit::Ether => U256::from(10).pow(U256::from(18)),
        }
    }
}

/// Parse a string like "5gwei" or "0.01ether" into Wei as U256
pub fn parse_eth_value(input: &str) -> Result<U256> {
    // Check if the input is a pure number
    if input.chars().all(|c| c.is_digit(10) || c == '.') {
        // Parse as Wei by default
        return Ok(input.parse::<U256>()?);
    }

    // Look for a number followed by a unit
    let mut numeric_part = String::new();
    let mut unit_part = String::new();
    let mut seen_dot = false;

    for c in input.chars() {
        if c.is_digit(10) {
            numeric_part.push(c);
        } else if c == '.' && !seen_dot {
            seen_dot = true;
            numeric_part.push(c);
        } else {
            unit_part.push(c);
        }
    }

    if numeric_part.is_empty() || unit_part.is_empty() {
        return Err(eyre!("Invalid format: expected '<number><unit>', got '{}'", input));
    }

    let unit = EthUnit::from_str(&unit_part)?;
    
    // Handle decimal values
    if seen_dot {
        let parts: Vec<&str> = numeric_part.split('.').collect();
        if parts.len() != 2 {
            return Err(eyre!("Invalid decimal format in '{}'", numeric_part));
        }
        
        let whole_part = parts[0].parse::<f64>().unwrap_or(0.0);
        let decimal_part = format!("0.{}", parts[1]).parse::<f64>().unwrap_or(0.0);
        let value = whole_part + decimal_part;
        
        // Convert to wei
        let multiplier = unit.multiplier();
        let value_wei = u256_from_f64_lossy(value) * multiplier;
        
        Ok(value_wei)
    } else {
        // Integer value
        let value: U256 = numeric_part.parse()?;
        Ok(value * unit.multiplier())
    }
}

/// Parses a value with an operator prefix like "ge5gwei" or "le0.01ether"
pub fn parse_value_with_operator(input: &str) -> Result<(String, U256)> {
    if input.len() < 4 {  // At minimum need "ge1" + something
        return Err(eyre!("Input too short: {}", input));
    }
    
    let operator = &input[0..2];
    if operator != "ge" && operator != "le" {
        return Err(eyre!("Invalid operator: must start with 'ge' or 'le'"));
    }
    
    let value_str = &input[2..];
    let value = parse_eth_value(value_str)?;
    
    Ok((operator.to_string(), value))
}

/// Create a U256 from an f64 value, potentially losing precision
pub fn u256_from_f64_lossy(value: f64) -> U256 {
    let value_string = format!("{:.0}", value);
    value_string.parse::<U256>().unwrap_or_else(|_| U256::from(value as u64))
}