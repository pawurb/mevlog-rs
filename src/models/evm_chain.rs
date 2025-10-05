use alloy_chains::NamedChain;
use eyre::Result;
use revm::primitives::Address;

use crate::models::db_chain::DBChain;

#[derive(Debug, Clone)]
pub struct EVMChain {
    pub chain_id: u64,
    pub name: String,
    pub explorer_url: Option<String>,
    pub currency_symbol: String,
    pub chainlink_oracle: Option<Address>,
    pub uniswap_v2_pool: Option<Address>,
    pub rpc_url: String,
}

impl EVMChain {
    pub fn new(db_chain: DBChain, rpc_url: String) -> Result<Self> {
        Ok(Self {
            chain_id: db_chain.id as u64,
            name: db_chain.name,
            explorer_url: db_chain.explorer_url,
            currency_symbol: db_chain.currency_symbol,
            chainlink_oracle: db_chain.chainlink_oracle.map(|s| {
                s.parse()
                    .unwrap_or_else(|_| panic!("Invalid chainlink oracle address {s}"))
            }),
            uniswap_v2_pool: db_chain.uniswap_v2_pool.map(|s| {
                s.parse()
                    .unwrap_or_else(|_| panic!("Invalid uniswap v2 pool address {s}"))
            }),
            rpc_url,
        })
    }

    pub fn revm_cache_dir_name(&self) -> String {
        if let Ok(chain) = NamedChain::try_from(self.chain_id) {
            chain.to_string()
        } else {
            format!("network_{}", self.chain_id)
        }
    }

    pub fn cryo_cache_dir_name(&self) -> &str {
        // Based on https://github.com/paradigmxyz/cryo/blob/559b65455d7ef6b03e8e9e96a0e50fd4fe8a9c86/crates/cli/src/parse/file_output.rs#L62
        match self.chain_id {
            1 => "ethereum",
            5 => "goerli",
            10 => "optimism",
            56 => "bnb",
            69 => "optimism_kovan",
            100 => "gnosis",
            137 => "polygon",
            420 => "optimism_goerli",
            1101 => "polygon_zkevm",
            1442 => "polygon_zkevm_testnet",
            8453 => "base",
            10200 => "gnosis_chidao",
            17000 => "holesky",
            42161 => "arbitrum",
            42170 => "arbitrum_nova",
            43114 => "avalanche",
            80001 => "polygon_mumbai",
            84531 => "base_goerli",
            7777777 => "zora",
            11155111 => "sepolia",
            _ => &self.name,
        }
    }

    pub fn is_mainnet(&self) -> bool {
        self.chain_id == 1
    }
}
