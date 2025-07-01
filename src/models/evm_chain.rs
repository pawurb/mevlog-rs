use std::{collections::HashMap, sync::OnceLock};

use eyre::Result;
use revm::primitives::{address, Address};
use tracing::warn;

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum EVMChainType {
    Mainnet,
    Base,
    BSC,
    Arbitrum,
    Polygon,
    Metis,
    Optimism,
    Avalanche,
    Linea,
    Scroll,
    Fantom,
    Unknown(u64),
}

#[derive(Debug, Clone)]
pub struct EVMChain {
    pub chain_type: EVMChainType,
    pub rpc_url: String,
}

impl EVMChainType {
    pub fn chain_id(&self) -> u64 {
        match self {
            EVMChainType::Mainnet => 1,
            EVMChainType::Base => 8453,
            EVMChainType::BSC => 56,
            EVMChainType::Arbitrum => 42161,
            EVMChainType::Polygon => 137,
            EVMChainType::Metis => 1088,
            EVMChainType::Optimism => 10,
            EVMChainType::Avalanche => 43114,
            EVMChainType::Linea => 59144,
            EVMChainType::Scroll => 534352,
            EVMChainType::Fantom => 250,
            EVMChainType::Unknown(chain_id) => *chain_id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            EVMChainType::Mainnet => "mainnet",
            EVMChainType::Base => "base",
            EVMChainType::BSC => "bsc",
            EVMChainType::Arbitrum => "arbitrum",
            EVMChainType::Polygon => "polygon",
            EVMChainType::Metis => "metis",
            EVMChainType::Optimism => "optimism",
            EVMChainType::Avalanche => "avalanche",
            EVMChainType::Linea => "linea",
            EVMChainType::Scroll => "scroll",
            EVMChainType::Fantom => "fantom",
            EVMChainType::Unknown(_) => "unknown",
        }
    }

    pub fn supported() -> Vec<Self> {
        vec![
            EVMChainType::Mainnet,
            EVMChainType::Base,
            EVMChainType::BSC,
            EVMChainType::Arbitrum,
            EVMChainType::Polygon,
            EVMChainType::Metis,
            EVMChainType::Optimism,
            EVMChainType::Avalanche,
            EVMChainType::Linea,
            EVMChainType::Scroll,
            EVMChainType::Fantom,
        ]
    }

    pub fn supported_chains_text() -> String {
        let chains = Self::supported()
            .iter()
            .map(|chain| format!("- {} ({})", chain.name(), chain.chain_id()))
            .collect::<Vec<_>>()
            .join("\n");

        format!(
            r#"Currently supported EVM chains:
{chains}
Visit https://github.com/pawurb/mevlog-rs/issues/9 to add more."#
        )
    }
}

impl EVMChain {
    pub fn new(chain_id: u64, rpc_url: String) -> Result<Self> {
        let supported_chains = EVMChainType::supported();
        let matching_chain = if let Some(chain) = supported_chains
            .iter()
            .find(|chain| chain.chain_id() == chain_id)
        {
            chain.clone()
        } else {
            warn!("Unknown chain id: {}. Using unknown chain type, functionality might be limited.\n{}",
                chain_id,
                EVMChainType::supported_chains_text()
            );
            EVMChainType::Unknown(chain_id)
        };

        Ok(Self {
            rpc_url,
            chain_type: matching_chain.clone(),
        })
    }

    pub fn chain_id(&self) -> u64 {
        self.chain_type.chain_id()
    }

    pub fn name(&self) -> &str {
        self.chain_type.name()
    }

    pub fn revm_cache_dir_name(&self) -> &str {
        self.name()
    }

    pub fn cryo_cache_dir_name(&self) -> String {
        // Based on https://github.com/paradigmxyz/cryo/blob/559b65455d7ef6b03e8e9e96a0e50fd4fe8a9c86/crates/cli/src/parse/file_output.rs#L62
        match self.chain_id() {
            1 => "ethereum".to_string(),
            5 => "goerli".to_string(),
            10 => "optimism".to_string(),
            56 => "bnb".to_string(),
            69 => "optimism_kovan".to_string(),
            100 => "gnosis".to_string(),
            137 => "polygon".to_string(),
            420 => "optimism_goerli".to_string(),
            1101 => "polygon_zkevm".to_string(),
            1442 => "polygon_zkevm_testnet".to_string(),
            8453 => "base".to_string(),
            10200 => "gnosis_chidao".to_string(),
            17000 => "holesky".to_string(),
            42161 => "arbitrum".to_string(),
            42170 => "arbitrum_nova".to_string(),
            43114 => "avalanche".to_string(),
            80001 => "polygon_mumbai".to_string(),
            84531 => "base_goerli".to_string(),
            7777777 => "zora".to_string(),
            11155111 => "sepolia".to_string(),
            chain_id => format!("network_{chain_id}"),
        }
    }

    // Gas token/USD price oracle
    // https://docs.chain.link/data-feeds/price-feeds/addresses
    pub fn price_oracle(&self) -> Address {
        match self.chain_type {
            EVMChainType::Mainnet => address!("0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"),
            EVMChainType::Base => address!("0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"),
            EVMChainType::BSC => address!("0x0567f2323251f0aab15c8dfb1967e4e8a7d42aee"),
            EVMChainType::Arbitrum => address!("0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612"),
            EVMChainType::Polygon => address!("0xAB594600376Ec9fD91F8e885dADF0CE036862dE0"),
            EVMChainType::Metis => address!("0xD4a5Bb03B5D66d9bf81507379302Ac2C2DFDFa6D"),
            EVMChainType::Optimism => address!("0x13e3Ee699D1909E989722E753853AE30b17e08c5"),
            EVMChainType::Avalanche => address!("0x0A77230d17318075983913bC2145DB16C7366156"),
            EVMChainType::Linea => address!("0x3c6Cd9Cc7c7a4c2Cf5a82734CD249D7D593354dA"),
            EVMChainType::Scroll => address!("0x6bF14CB0A831078629D993FDeBcB182b21A8774C"),
            EVMChainType::Fantom => address!("0x11DdD3d147E5b83D01cee7070027092397d63658"),
            EVMChainType::Unknown(_) => address!("0x0000000000000000000000000000000000000000"),
        }
    }

    pub fn etherscan_url(&self) -> &str {
        match self.chain_type {
            EVMChainType::Mainnet => "https://etherscan.io",
            EVMChainType::Base => "https://basescan.org",
            EVMChainType::BSC => "https://bscscan.com",
            EVMChainType::Arbitrum => "https://arbiscan.io",
            EVMChainType::Polygon => "https://polygonscan.com",
            EVMChainType::Metis => "https://andromeda-explorer.metis.io",
            EVMChainType::Optimism => "https://optimistic.etherscan.io",
            EVMChainType::Avalanche => "https://snowtrace.io",
            EVMChainType::Linea => "https://lineascan.build",
            EVMChainType::Scroll => "https://scrollscan.com",
            EVMChainType::Fantom => "https://explorer.fantom.network",
            EVMChainType::Unknown(_) => "https://etherscan.io",
        }
    }

    pub fn currency_symbol(&self) -> &str {
        match self.chain_type {
            EVMChainType::BSC => "BNB",
            EVMChainType::Polygon => "POL",
            EVMChainType::Avalanche => "AVAX",
            EVMChainType::Metis => "METIS",
            EVMChainType::Fantom => "FTM",
            _ => "ETH",
        }
    }

    // Common signatures, that are duplicate and mismatched in the database
    // (signature_hash, tx_index) -> name_override
    pub fn signature_overrides(&self) -> &HashMap<(String, u64), String> {
        type SignatureMap = HashMap<(String, u64), String>;
        type ChainOverrides = HashMap<EVMChainType, SignatureMap>;

        static SIGNATURE_OVERRIDES: OnceLock<ChainOverrides> = OnceLock::new();
        static EMPTY_MAP: OnceLock<SignatureMap> = OnceLock::new();

        let overrides = SIGNATURE_OVERRIDES.get_or_init(|| {
            let mut map = HashMap::new();
            map.insert(
                EVMChainType::Base,
                HashMap::from([(
                    ("0x098999be".to_string(), 0),
                    "setL1BlockValuesIsthmus()".to_string(),
                )]),
            );
            map
        });

        overrides
            .get(&self.chain_type)
            .unwrap_or_else(|| EMPTY_MAP.get_or_init(HashMap::new))
    }
}
