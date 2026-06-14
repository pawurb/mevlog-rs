use std::{collections::HashMap, fs, path::PathBuf};

use eyre::{Result, bail};
use revm::primitives::{Address, FixedBytes};
use serde::{Deserialize, Serialize};

use crate::misc::shared_init::config_path;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    chains: HashMap<String, ChainConfig>,
    #[serde(default)]
    tables: HashMap<String, CustomTableConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainConfig {
    pub rpc_url: String,
}

/// Raw `[tables.<name>]` config entry: a custom txs-DB table populated from
/// `logs` rows matching `topic0`, with topics / data byte ranges mapped to
/// columns. Validated and parsed into a [`CustomTable`] at config load.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CustomTableConfig {
    topic0: String,
    /// Chain IDs the table applies to. `None` means all chains.
    chains: Option<Vec<u64>>,
    /// Optional emitter filter; only logs from these addresses are captured.
    addresses: Option<Vec<String>>,
    columns: Vec<CustomColumnConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CustomColumnConfig {
    name: String,
    /// `topic1`..`topic3`, or a 0-based end-exclusive data byte range like
    /// `data[0:32]` (ABI word *n* is `data[n*32:(n+1)*32]`).
    source: String,
    r#type: String,
}

/// Validated form of [`CustomTableConfig`]; the only shape the DB layer
/// consumes. Table and column names are guaranteed safe to interpolate into
/// SQL identifiers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomTable {
    pub(crate) name: String,
    pub(crate) topic0: FixedBytes<32>,
    pub(crate) chains: Option<Vec<u64>>,
    /// Empty means no emitter filter.
    pub(crate) addresses: Vec<Address>,
    pub(crate) columns: Vec<CustomColumn>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomColumn {
    pub(crate) name: String,
    pub(crate) source: ColumnSource,
    pub(crate) r#type: ColumnType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ColumnSource {
    /// `topic1`..`topic3` (`topic0` is the table's match key, not a source).
    Topic(u8),
    /// 0-based, end-exclusive byte range into `data`.
    Data { start: usize, end: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnType {
    /// 20-byte BLOB; 32-byte sources get the 12-byte ABI pad stripped.
    Address,
    /// 32-byte big-endian BLOB; shorter data ranges are left-padded so the
    /// `u256_*` SQLite functions and blob comparisons keep working.
    Uint256,
    /// Raw BLOB, source slice stored verbatim.
    Bytes,
}

/// Table names that would collide with the txs DB schema or SQLite internals.
const RESERVED_TABLE_NAMES: &[&str] = &[
    "transactions",
    "blocks",
    "logs",
    "custom_tables",
    "_sqlx_migrations",
];

/// Columns every custom table gets implicitly; config columns must not shadow
/// them.
const IMPLICIT_COLUMN_NAMES: &[&str] = &["block_number", "tx_index", "log_index", "address"];

/// `^[a-z_][a-z0-9_]*$` — safe to interpolate as a SQL identifier.
pub(crate) fn valid_sql_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

fn parse_hex_bytes(value: &str, expected_len: usize, what: &str) -> Result<Vec<u8>> {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    let bytes =
        hex::decode(stripped).map_err(|e| eyre::eyre!("invalid hex in {what} '{value}': {e}"))?;
    if bytes.len() != expected_len {
        bail!(
            "{what} '{value}' must be {expected_len} bytes, got {}",
            bytes.len()
        );
    }
    Ok(bytes)
}

impl ColumnSource {
    fn parse(source: &str) -> Result<Self> {
        if let Some(rest) = source.strip_prefix("topic") {
            let idx: u8 = rest
                .parse()
                .map_err(|_| eyre::eyre!("invalid column source '{source}'"))?;
            if !(1..=3).contains(&idx) {
                bail!("invalid column source '{source}': topic index must be 1..=3");
            }
            return Ok(Self::Topic(idx));
        }

        if let Some(range) = source
            .strip_prefix("data[")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            let Some((start, end)) = range.split_once(':') else {
                bail!("invalid column source '{source}': expected data[start:end]");
            };
            let start: usize = start
                .trim()
                .parse()
                .map_err(|_| eyre::eyre!("invalid column source '{source}'"))?;
            let end: usize = end
                .trim()
                .parse()
                .map_err(|_| eyre::eyre!("invalid column source '{source}'"))?;
            if end <= start {
                bail!("invalid column source '{source}': end must be greater than start");
            }
            return Ok(Self::Data { start, end });
        }

        bail!("invalid column source '{source}': expected topic1..topic3 or data[start:end]")
    }

    /// Normalized form used for fingerprinting (whitespace variants collapse).
    pub(crate) fn canonical(&self) -> String {
        match self {
            Self::Topic(idx) => format!("topic{idx}"),
            Self::Data { start, end } => format!("data[{start}:{end}]"),
        }
    }
}

impl ColumnType {
    fn parse(r#type: &str) -> Result<Self> {
        match r#type {
            "address" => Ok(Self::Address),
            "uint256" => Ok(Self::Uint256),
            "bytes" => Ok(Self::Bytes),
            other => bail!("invalid column type '{other}': expected address, uint256 or bytes"),
        }
    }

    pub(crate) fn canonical(&self) -> &'static str {
        match self {
            Self::Address => "address",
            Self::Uint256 => "uint256",
            Self::Bytes => "bytes",
        }
    }
}

impl CustomColumn {
    fn from_config(table_name: &str, config: &CustomColumnConfig) -> Result<Self> {
        let ctx = format!("table '{table_name}' column '{}'", config.name);

        if !valid_sql_name(&config.name) {
            bail!("{ctx}: name must match ^[a-z_][a-z0-9_]*$");
        }
        if IMPLICIT_COLUMN_NAMES.contains(&config.name.as_str()) {
            bail!("{ctx}: name collides with an implicit column");
        }

        let source = ColumnSource::parse(&config.source).map_err(|e| eyre::eyre!("{ctx}: {e}"))?;
        let r#type = ColumnType::parse(&config.r#type).map_err(|e| eyre::eyre!("{ctx}: {e}"))?;

        if let ColumnSource::Data { start, end } = source {
            let len = end - start;
            match r#type {
                ColumnType::Address if len != 20 && len != 32 => {
                    bail!("{ctx}: address requires a 20- or 32-byte data range, got {len}")
                }
                ColumnType::Uint256 if len > 32 => {
                    bail!("{ctx}: uint256 requires a data range of at most 32 bytes, got {len}")
                }
                _ => {}
            }
        } else if r#type == ColumnType::Bytes {
            bail!("{ctx}: bytes requires a data range source");
        }

        Ok(Self {
            name: config.name.clone(),
            source,
            r#type,
        })
    }
}

impl CustomTable {
    fn from_config(name: &str, config: &CustomTableConfig) -> Result<Self> {
        if !valid_sql_name(name) {
            bail!("custom table name '{name}' must match ^[a-z_][a-z0-9_]*$");
        }
        if RESERVED_TABLE_NAMES.contains(&name) || name.starts_with("sqlite_") {
            bail!("custom table name '{name}' is reserved");
        }
        if config.columns.is_empty() {
            bail!("custom table '{name}' must define at least one column");
        }

        let topic0 = FixedBytes::<32>::from_slice(&parse_hex_bytes(
            &config.topic0,
            32,
            &format!("table '{name}' topic0"),
        )?);

        let addresses = config
            .addresses
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|addr| {
                Ok(Address::from_slice(&parse_hex_bytes(
                    addr,
                    20,
                    &format!("table '{name}' addresses entry"),
                )?))
            })
            .collect::<Result<Vec<_>>>()?;

        let columns = config
            .columns
            .iter()
            .map(|col| CustomColumn::from_config(name, col))
            .collect::<Result<Vec<_>>>()?;

        let mut seen = std::collections::HashSet::new();
        for col in &columns {
            if !seen.insert(col.name.as_str()) {
                bail!("custom table '{name}' has duplicate column '{}'", col.name);
            }
        }

        Ok(Self {
            name: name.to_string(),
            topic0,
            chains: config.chains.clone(),
            addresses,
            columns,
        })
    }

    pub(crate) fn applies_to_chain(&self, chain_id: u64) -> bool {
        match &self.chains {
            Some(chains) => chains.contains(&chain_id),
            None => true,
        }
    }
}

impl Config {
    pub(crate) fn config_file_path() -> PathBuf {
        config_path().join("config.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_file_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)?;
        let config: Self = toml::from_str(&content)?;
        // Fail fast on invalid [tables.*] entries — their names get
        // interpolated into SQL, so a bad config must never reach the DB layer.
        config.custom_tables()?;
        Ok(config)
    }

    /// Validated custom table definitions, sorted by name for deterministic
    /// processing order (TOML map order is not preserved).
    pub(crate) fn custom_tables(&self) -> Result<Vec<CustomTable>> {
        let mut names: Vec<&String> = self.tables.keys().collect();
        names.sort();
        names
            .into_iter()
            .map(|name| CustomTable::from_config(name, &self.tables[name]))
            .collect()
    }

    pub(crate) fn init_if_missing() -> Result<()> {
        let path = Self::config_file_path();
        if !path.exists() {
            fs::create_dir_all(config_path())?;
            fs::write(&path, Self::default_config_template())?;
        }
        Ok(())
    }

    fn default_config_template() -> &'static str {
        r#"# mevlog configuration file
#
# Configure custom RPC endpoints for each chain by chain ID.
# Uncomment and modify the examples below as needed.
#
# [chains.1]
# rpc_url = "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
#
# [chains.42161]
# rpc_url = "https://arb-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
#
# Custom tables in the local txs database, populated from indexed logs
# matching topic0. Columns map topics or data byte ranges (0-based,
# end-exclusive; ABI word n is data[n*32:(n+1)*32]) to SQLite columns.
# Types: address (20-byte BLOB), uint256 (32-byte big-endian BLOB, works
# with u256_sum/format_ether), bytes (verbatim slice). Note: dynamic ABI
# params (string/bytes) occupy their data slot as an offset, not a value —
# a range targeting such a slot stores the offset word.
# After editing a table's definition, rebuild it with:
# mevlog update-custom-tables --chain-id <id>
#
# Example (Uniswap V2 Swap):
#
# [tables.swaps]
# topic0 = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822"
# chains = [1]                                                # optional; default: all chains
# addresses = ["0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc"]  # optional emitter filter
#
# [[tables.swaps.columns]]
# name = "sender"
# source = "topic1"
# type = "address"
#
# [[tables.swaps.columns]]
# name = "amount0_in"
# source = "data[0:32]"
# type = "uint256"
#
# [[tables.swaps.columns]]
# name = "to_address"
# source = "topic2"
# type = "address"
"#
    }

    pub fn get_chain(&self, chain_id: u64) -> Option<&ChainConfig> {
        let key = chain_id.to_string();
        self.chains.get(&key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_unquoted() {
        let content = r#"
[chains.1]
rpc_url = "https://example.com"
"#;
        let config: Config = toml::from_str(content).unwrap();
        assert!(config.get_chain(1).is_some());
    }

    #[test]
    fn test_parse_quoted() {
        let content = r#"
[chains."1"]
rpc_url = "https://example.com"
"#;
        let config: Config = toml::from_str(content).unwrap();
        assert!(config.get_chain(1).is_some());
    }

    const SWAP_TOPIC0: &str = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822";

    fn swaps_toml(columns: &str) -> String {
        format!(
            r#"
[tables.swaps]
topic0 = "{SWAP_TOPIC0}"
chains = [1]
addresses = ["0xb4e16d0168e52d35cacd2c6185b44281ec28c9dc"]
{columns}
"#
        )
    }

    fn custom_tables(content: &str) -> Result<Vec<CustomTable>> {
        let config: Config = toml::from_str(content).unwrap();
        config.custom_tables()
    }

    #[test]
    fn parses_example_custom_table() {
        let content = swaps_toml(
            r#"
[[tables.swaps.columns]]
name = "sender"
source = "topic1"
type = "address"

[[tables.swaps.columns]]
name = "amount0_in"
source = "data[0:32]"
type = "uint256"

[[tables.swaps.columns]]
name = "raw"
source = "data[32:96]"
type = "bytes"
"#,
        );

        let tables = custom_tables(&content).unwrap();
        assert_eq!(tables.len(), 1);

        let table = &tables[0];
        assert_eq!(table.name, "swaps");
        assert_eq!(
            hex::encode(table.topic0),
            SWAP_TOPIC0.trim_start_matches("0x")
        );
        assert_eq!(table.chains, Some(vec![1]));
        assert_eq!(
            hex::encode(table.addresses[0].as_slice()),
            "b4e16d0168e52d35cacd2c6185b44281ec28c9dc"
        );
        assert!(table.applies_to_chain(1));
        assert!(!table.applies_to_chain(10));

        assert_eq!(
            table.columns[0],
            CustomColumn {
                name: "sender".to_string(),
                source: ColumnSource::Topic(1),
                r#type: ColumnType::Address,
            }
        );
        assert_eq!(
            table.columns[1].source,
            ColumnSource::Data { start: 0, end: 32 }
        );
        assert_eq!(
            table.columns[2],
            CustomColumn {
                name: "raw".to_string(),
                source: ColumnSource::Data { start: 32, end: 96 },
                r#type: ColumnType::Bytes,
            }
        );
    }

    #[test]
    fn no_chains_filter_applies_everywhere() {
        let content = format!(
            r#"
[tables.swaps]
topic0 = "{SWAP_TOPIC0}"

[[tables.swaps.columns]]
name = "sender"
source = "topic1"
type = "address"
"#
        );
        let tables = custom_tables(&content).unwrap();
        assert!(tables[0].applies_to_chain(1));
        assert!(tables[0].applies_to_chain(42161));
        assert!(tables[0].addresses.is_empty());
    }

    #[test]
    fn rejects_invalid_definitions() {
        // (columns toml, expected error fragment)
        let cases = [
            (
                "name = \"sender\"\nsource = \"topic4\"\ntype = \"address\"",
                "topic index must be 1..=3",
            ),
            (
                "name = \"sender\"\nsource = \"topic0\"\ntype = \"address\"",
                "topic index must be 1..=3",
            ),
            (
                "name = \"sender\"\nsource = \"data[0:33]\"\ntype = \"address\"",
                "20- or 32-byte data range",
            ),
            (
                "name = \"amount\"\nsource = \"data[0:64]\"\ntype = \"uint256\"",
                "at most 32 bytes",
            ),
            (
                "name = \"amount\"\nsource = \"data[32:32]\"\ntype = \"uint256\"",
                "end must be greater than start",
            ),
            (
                "name = \"raw\"\nsource = \"topic1\"\ntype = \"bytes\"",
                "bytes requires a data range",
            ),
            (
                "name = \"sender\"\nsource = \"topic1\"\ntype = \"uint128\"",
                "invalid column type",
            ),
            (
                "name = \"block_number\"\nsource = \"topic1\"\ntype = \"address\"",
                "implicit column",
            ),
            (
                "name = \"Sender\"\nsource = \"topic1\"\ntype = \"address\"",
                "must match",
            ),
        ];

        for (columns, expected) in cases {
            let content = swaps_toml(&format!("[[tables.swaps.columns]]\n{columns}"));
            let err = custom_tables(&content).unwrap_err().to_string();
            assert!(
                err.contains(expected),
                "'{err}' should contain '{expected}'"
            );
        }
    }

    #[test]
    fn rejects_reserved_and_invalid_table_names() {
        for name in ["logs", "custom_tables", "sqlite_foo", "Swaps", "a-b"] {
            let content = format!(
                r#"
[tables.{name}]
topic0 = "{SWAP_TOPIC0}"

[[tables.{name}.columns]]
name = "sender"
source = "topic1"
type = "address"
"#,
                name = format!("\"{name}\"")
            );
            assert!(
                custom_tables(&content).is_err(),
                "table name '{name}' should be rejected"
            );
        }
    }

    #[test]
    fn rejects_duplicate_columns_and_bad_hex() {
        let dup = swaps_toml(
            r#"
[[tables.swaps.columns]]
name = "sender"
source = "topic1"
type = "address"

[[tables.swaps.columns]]
name = "sender"
source = "topic2"
type = "address"
"#,
        );
        assert!(
            custom_tables(&dup)
                .unwrap_err()
                .to_string()
                .contains("duplicate column")
        );

        let bad_topic0 = r#"
[tables.swaps]
topic0 = "0x1234"

[[tables.swaps.columns]]
name = "sender"
source = "topic1"
type = "address"
"#;
        assert!(
            custom_tables(bad_topic0)
                .unwrap_err()
                .to_string()
                .contains("32 bytes")
        );
    }
}
