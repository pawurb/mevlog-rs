use crate::misc::utils::UNKNOWN;

use super::db_event::DBEvent;
use alloy::rpc::types::Log as AlloyLog;
use colored::Colorize;
use eyre::Result;
use revm::primitives::{Address, FixedBytes};
use sqlx::SqlitePool;
use std::fmt;

#[derive(Debug)]
pub struct MEVLog {
    source: Address,
    pub signature: String,
    inner: AlloyLog,
}

impl MEVLog {
    pub async fn new(
        first_topic: &FixedBytes<32>,
        inner_log: AlloyLog,
        sqlite: &SqlitePool,
    ) -> Result<Self> {
        let signature = DBEvent::find_by_hash(&format!("{}", first_topic), sqlite).await?;

        Ok(Self {
            source: inner_log.inner.address,
            signature: signature.unwrap_or(UNKNOWN.to_string()),
            inner: inner_log,
        })
    }

    pub fn source(&self) -> Address {
        self.source
    }
}

impl fmt::Display for MEVLog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {}", "emit".yellow(), self.signature.blue())?;
        for (i, topic) in self.inner.topics().iter().enumerate() {
            if i != 0 {
                writeln!(f, "      {:?}", topic)?;
            }
        }
        Ok(())
    }
}
