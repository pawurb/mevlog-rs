use std::fmt;

use eyre::Result;
use revm::primitives::{Address, U256};

use crate::misc::{
    symbol_utils::{ERC20SymbolsLookup, symbol_lookup_async, symbol_lookup_only_cached},
    utils::UNKNOWN,
};

#[derive(Debug, Clone)]
pub struct MEVLogSignature {
    pub signature: String,
    pub symbol: Option<String>,
    pub amount: Option<U256>,
    show_amount: bool,
}

#[derive(Debug, Clone)]
pub enum MEVLogSignatureType {
    ERC20,
    UNIV2,
    UNIV3,
}

impl MEVLogSignature {
    pub async fn new(
        address: Address,
        signature_str: Option<String>,
        symbols_lookup: &ERC20SymbolsLookup,
        show_amount: bool,
    ) -> Result<Self> {
        let signature_str = signature_str.unwrap_or(UNKNOWN.to_string());
        let signature_type = get_signature_type(&signature_str);

        let symbol = match symbols_lookup {
            ERC20SymbolsLookup::Async(symbols_lookup) => {
                symbol_lookup_async(address, signature_type, symbols_lookup).await?
            }
            ERC20SymbolsLookup::OnlyCached => symbol_lookup_only_cached(address).await?,
        };

        Ok(Self {
            signature: signature_str,
            symbol,
            amount: None,
            show_amount,
        })
    }

    pub fn with_amount(mut self, amount: Option<U256>) -> Self {
        self.amount = amount;
        self
    }
}

fn get_signature_type(signature_str: &str) -> Option<MEVLogSignatureType> {
    match signature_str {
        "Transfer(address,address,uint256)" => Some(MEVLogSignatureType::ERC20),
        "Approval(address,address,uint256)" => Some(MEVLogSignatureType::ERC20),
        "Swap(address,uint256,uint256,uint256,uint256,address)" => Some(MEVLogSignatureType::UNIV2),
        "Swap(address,address,int256,int256,uint160,uint128,int24)" => {
            Some(MEVLogSignatureType::UNIV3)
        }
        _ => None,
    }
}

impl fmt::Display for MEVLogSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(amount) = self.amount {
            if self.show_amount {
                write!(
                    f,
                    "{} {} {}",
                    self.signature,
                    amount,
                    self.symbol.as_deref().unwrap_or_default()
                )
            } else {
                write!(
                    f,
                    "{} {}",
                    self.signature,
                    self.symbol.as_deref().unwrap_or_default()
                )
            }
        } else {
            write!(
                f,
                "{} {}",
                self.signature,
                self.symbol.as_deref().unwrap_or_default()
            )
        }
    }
}
