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

#[hotpath::measure_all]
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

    pub fn cryo_cache_dir_name(&self) -> String {
        // Based on https://github.com/paradigmxyz/cryo/blob/559b65455d7ef6b03e8e9e96a0e50fd4fe8a9c86/crates/cli/src/parse/file_output.rs#L62
        match self.chain_id {
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

    pub fn is_mainnet(&self) -> bool {
        self.chain_id == 1
    }
}
