use std::fmt;

use colored::Colorize;
use eyre::Result;
use revm::primitives::{Address, FixedBytes, U256};
use sqlx::SqlitePool;

use super::{db_event::DBEvent, mev_log_signature::MEVLogSignature};
use crate::misc::symbol_utils::ERC20SymbolsLookup;

#[derive(Debug)]
pub struct MEVLog {
    pub source: Address,
    pub signature: MEVLogSignature,
    pub topics: Vec<FixedBytes<32>>,
    pub data: Vec<u8>,
    pub tx_index: u64,
}

impl MEVLog {
    // CSV row:
    // block_number 0
    // transaction_index 1
    // log_index 2
    // transaction_hash 3
    // address 4
    // topic0 5
    // topic1 6
    // topic2 7
    // topic3 8
    // data 9
    // chain_id 10
    pub async fn from_csv_row(
        record: &csv::StringRecord,
        symbols_lookup: &ERC20SymbolsLookup,
        sqlite: &SqlitePool,
        show_erc20_transfer_amount: bool,
    ) -> Result<Self> {
        let first_topic = record[5].to_string();
        let data = record[9].to_string();

        let signature_str = DBEvent::find_by_hash(&first_topic, sqlite).await?;
        let data = hex::decode(data.strip_prefix("0x").unwrap_or(&data))?;
        let source: Address = record[4].parse()?;
        let signature = MEVLogSignature::new(
            source,
            signature_str.clone(),
            symbols_lookup,
            show_erc20_transfer_amount,
        )
        .await?;

        let topics = [
            record[5].to_string(),
            record[6].to_string(),
            record[7].to_string(),
            record[8].to_string(),
        ]
        .iter()
        .filter_map(|s| {
            if s.is_empty() {
                None
            } else {
                Some(FixedBytes::from_slice(
                    &hex::decode(s.strip_prefix("0x").unwrap_or(s)).unwrap(),
                ))
            }
        })
        .collect::<Vec<_>>();
        let tx_index = record[1].parse()?;
        let log = Self {
            source,
            signature,
            topics: topics.clone(),
            data: data.clone(),
            tx_index,
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
                source,
                signature,
                topics,
                data,
                tx_index,
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
