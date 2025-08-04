use std::fmt;

use alloy::rpc::types::Log as AlloyLog;
use colored::Colorize;
use eyre::Result;
use revm::primitives::{Address, FixedBytes, U256};
use sqlx::SqlitePool;

use super::{db_event::DBEvent, mev_log_signature::MEVLogSignature};
use crate::misc::symbol_utils::SymbolLookupWorker;

#[derive(Debug)]
pub struct MEVLog {
    pub source: Address,
    pub signature: MEVLogSignature,
    pub topics: Vec<FixedBytes<32>>,
    pub data: Vec<u8>,
}

impl MEVLog {
    pub async fn new(
        first_topic: &FixedBytes<32>,
        inner_log: &AlloyLog,
        symbols_lookup_worker: &SymbolLookupWorker,
        sqlite: &SqlitePool,
        show_erc20_transfer_amount: bool,
    ) -> Result<Self> {
        let signature_str = DBEvent::find_by_hash(&format!("{first_topic}"), sqlite).await?;
        let data = inner_log.inner.data.data.to_vec();

        let signature = MEVLogSignature::new(
            inner_log.inner.address,
            signature_str.clone(),
            symbols_lookup_worker,
            show_erc20_transfer_amount,
        )
        .await?;

        let log = Self {
            source: inner_log.inner.address,
            signature,
            topics: inner_log.topics().to_vec(),
            data: data.clone(),
        };

        if log.is_erc20_transfer() {
            let amount = if data.len() >= 32 {
                let amount_bytes: [u8; 32] = data[..32].try_into().ok().unwrap_or([0; 32]);
                Some(U256::from_be_bytes(amount_bytes))
            } else {
                None
            };

            let signature = log.signature.with_amount(amount);

            return Ok(Self {
                source: log.source,
                signature,
                topics: log.topics,
                data: log.data,
            });
        }

        Ok(log)
    }

    pub fn source(&self) -> Address {
        self.source
    }

    pub fn is_erc20_transfer(&self) -> bool {
        self.signature.signature == "Transfer(address,address,uint256)" && !self.data.is_empty()
    }
}

impl fmt::Display for MEVLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "{} {}",
            "emit".yellow(),
            format!("{}", self.signature).blue()
        )?;
        for (i, topic) in self.topics.iter().enumerate() {
            if i != 0 {
                writeln!(f, "      {topic:?}")?;
            }
        }
        Ok(())
    }
}
