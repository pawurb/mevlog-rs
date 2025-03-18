use crate::misc::symbol_utils::SymbolLookupWorker;

use super::{db_event::DBEvent, mev_log_signature::MEVLogSignature};
use alloy::rpc::types::Log as AlloyLog;
use colored::Colorize;
use eyre::Result;
use revm::primitives::{Address, FixedBytes};
use sqlx::SqlitePool;
use std::fmt;

#[derive(Debug)]
pub struct MEVLog {
    source: Address,
    pub signature: MEVLogSignature,
    topics: Vec<FixedBytes<32>>,
}

impl MEVLog {
    pub async fn new(
        first_topic: &FixedBytes<32>,
        inner_log: &AlloyLog,
        symbols_lookup_worker: &SymbolLookupWorker,
        sqlite: &SqlitePool,
    ) -> Result<Self> {
        let signature_str = DBEvent::find_by_hash(&format!("{}", first_topic), sqlite).await?;

        let signature = MEVLogSignature::new(
            inner_log.inner.address,
            signature_str,
            symbols_lookup_worker,
        )
        .await?;

        Ok(Self {
            source: inner_log.inner.address,
            signature,
            topics: inner_log.topics().to_vec(),
        })
    }

    pub fn source(&self) -> Address {
        self.source
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
                writeln!(f, "      {:?}", topic)?;
            }
        }
        Ok(())
    }
}
