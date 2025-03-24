use eyre::{bail, Result};
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
            _ => bail!("Unknown unit: {}", s),
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
#[allow(clippy::result_large_err)]
pub fn parse_eth_value(input: &str) -> Result<U256> {
    // Check if the input is a pure number
    if input.chars().all(|c| c.is_ascii_digit() || c == '.') {
        // Parse as Wei by default
        return parse_decimal_value(input, EthUnit::Wei);
    }

    // Extract numeric and unit parts
    let mut numeric_part = String::new();
    let mut unit_part = String::new();
    let mut in_unit_part = false;

    for c in input.chars() {
        if !in_unit_part && (c.is_ascii_digit() || c == '.') {
            numeric_part.push(c);
        } else {
            in_unit_part = true;
            unit_part.push(c);
        }
    }

    if numeric_part.is_empty() || unit_part.is_empty() {
        bail!("Invalid format: expected '<number><unit>', got '{}'", input)
    }

    let unit = EthUnit::from_str(&unit_part)?;
    parse_decimal_value(&numeric_part, unit)
}

fn parse_decimal_value(value_str: &str, unit: EthUnit) -> Result<U256> {
    if !value_str.contains('.') {
        // Integer value
        let value: U256 = value_str.parse()?;
        return Ok(value * unit.multiplier());
    }

    let parts: Vec<&str> = value_str.split('.').collect();
    if parts.len() != 2 {
        bail!("Invalid decimal format in '{}'", value_str)
    }

    let whole_part: U256 = if parts[0].is_empty() {
        U256::from(0)
    } else {
        parts[0].parse()?
    };

    // Calculate the decimal part with proper scaling
    let decimal_str = parts[1];

    if !decimal_str.is_empty() {
        // Prevent overflows by limiting decimal precision
        let max_decimal_len = 77; // U256 can represent approximately 77 decimal digits
        let limited_decimal = if decimal_str.len() > max_decimal_len {
            &decimal_str[0..max_decimal_len]
        } else {
            decimal_str
        };

        let decimal_part: U256 = limited_decimal.parse()?;

        // Calculate decimal scaling factor
        let decimal_scale = U256::from(10).pow(U256::from(limited_decimal.len()));

        // Apply unit multiplier to whole and decimal parts separately
        let whole_in_wei = whole_part * unit.multiplier();
        let decimal_in_wei = decimal_part * unit.multiplier() / decimal_scale;

        return Ok(whole_in_wei + decimal_in_wei);
    }

    // Just whole part
    Ok(whole_part * unit.multiplier())
}

/// Create a U256 from an f64 value, potentially losing precision
pub fn u256_from_f64_lossy(value: f64) -> U256 {
    let value_string = format!("{:.0}", value);
    value_string
        .parse::<U256>()
        .unwrap_or_else(|_| U256::from(value as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_eth_value() {
        // Test wei values
        assert_eq!(
            parse_eth_value("100").unwrap(),
            U256::from(100),
            "Should parse raw integer as wei"
        );

        // Test gwei values
        assert_eq!(
            parse_eth_value("5gwei").unwrap(),
            U256::from(5) * U256::from(10).pow(U256::from(9)),
            "Should convert 5 gwei to wei correctly"
        );

        // Test ether values
        assert_eq!(
            parse_eth_value("1ether").unwrap(),
            U256::from(10).pow(U256::from(18)),
            "Should convert 1 ether to wei correctly"
        );

        assert_eq!(
            parse_eth_value("0.5ether").unwrap(),
            U256::from(10).pow(U256::from(18)) / U256::from(2),
            "Should convert 0.5 ether to wei correctly"
        );
    }
}
