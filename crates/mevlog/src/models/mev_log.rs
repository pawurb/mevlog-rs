use arrow::record_batch::RecordBatch;
use eyre::Result;
use revm::primitives::{Address, FixedBytes, U256};
use sqlx::SqlitePool;

use super::mev_log_signature::MEVLogSignature;
use crate::{db::sigs::models::event::Event, misc::parquet_utils::get_parquet_string_value};

#[derive(Debug, Clone)]
pub struct MEVLog {
    pub source: Address,
    pub signature: MEVLogSignature,
    pub topics: Vec<FixedBytes<32>>,
    pub data: Vec<u8>,
    pub tx_index: u64,
    pub log_index: u64,
    pub tx_hash: FixedBytes<32>,
}

#[hotpath::measure_all(future = true)]
impl MEVLog {
    // Parquet row:
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
    pub async fn from_parquet_row(
        batch: &RecordBatch,
        row_idx: usize,
        sqlite: &SqlitePool,
    ) -> Result<(Self, u64)> {
        let get_string_value =
            |col_idx: usize| -> String { get_parquet_string_value(batch, col_idx, row_idx) };

        let block_number = get_string_value(0).parse::<u64>().unwrap();

        let first_topic = get_string_value(5);
        let data = get_string_value(9);

        let signature_str = Event::find_by_topic(&first_topic, sqlite).await?;
        let data = hex::decode(data.strip_prefix("0x").unwrap_or(&data))?;
        let source: Address = get_string_value(4).parse()?;
        let signature = MEVLogSignature::new(signature_str.clone());

        let topics = [
            get_string_value(5),
            get_string_value(6),
            get_string_value(7),
            get_string_value(8),
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
        let tx_index = get_string_value(1).parse()?;
        let log_index = get_string_value(2).parse()?;
        let tx_hash_str = get_string_value(3);
        let tx_hash = FixedBytes::<32>::from_slice(&hex::decode(
            tx_hash_str.strip_prefix("0x").unwrap_or(&tx_hash_str),
        )?);
        let log = Self {
            source,
            signature,
            topics: topics.clone(),
            data: data.clone(),
            tx_index,
            log_index,
            tx_hash,
        };

        if log.is_erc20_transfer() {
            let amount = if data.len() >= 32 {
                let amount_bytes: [u8; 32] = data[..32].try_into().ok().unwrap_or([0; 32]);
                Some(U256::from_be_bytes(amount_bytes))
            } else {
                None
            };

            let signature = log.signature.with_amount(amount);

            return Ok((
                Self {
                    source,
                    signature,
                    topics,
                    data,
                    tx_index,
                    log_index,
                    tx_hash,
                },
                block_number,
            ));
        }

        Ok((log, block_number))
    }

    pub fn source(&self) -> Address {
        self.source
    }

    pub fn is_erc20_transfer(&self) -> bool {
        self.signature.signature == "Transfer(address,address,uint256)" && !self.data.is_empty()
    }
}
