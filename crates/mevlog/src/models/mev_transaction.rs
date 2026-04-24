use std::{
    ops::{Add, Div},
    str::FromStr,
    sync::Arc,
};

use alloy::{
    rlp::Encodable,
    rpc::types::{AccessList, TransactionInput, TransactionRequest},
};
use bigdecimal::{BigDecimal, ToPrimitive};
use eyre::Result;
use revm::primitives::{Address, Bytes, FixedBytes, TxKind, U256, keccak256};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use super::{
    db_method::DBMethod, mev_address::MEVAddress, mev_block::TxData, mev_log::MEVLog,
    mev_log_group::MEVLogGroup, mev_opcode::MEVOpcode, mev_state_diff::MEVStateDiff,
};
use crate::{
    GenericProvider,
    misc::{
        ens_utils::ENSLookup,
        parquet_utils::get_parquet_string_value,
        utils::{ETH_TRANSFER, UNKNOWN, wei_to_eth},
    },
    models::evm_chain::EVMChain,
};

#[derive(Debug, Clone)]
pub struct ReceiptData {
    pub success: bool,
    pub effective_gas_price: u128,
    pub gas_used: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallExtract {
    pub from: Address,
    pub to: Address,
    pub signature: String,
    pub signature_hash: Option<String>,
}

#[derive(Debug)]
pub struct MEVTransaction {
    pub block_number: u64,
    pub native_token_price: Option<f64>,
    pub chain: Arc<EVMChain>,
    pub signature: String,
    pub signature_hash: Option<String>,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    pub inner: TransactionRequest,
    log_groups: Vec<MEVLogGroup>,
    source: MEVAddress,
    target: Option<MEVAddress>,
    pub to: TxKind,
    pub nonce: u64,
    pub coinbase_transfer: Option<U256>,
    pub receipt: ReceiptData,
    pub top_metadata: bool,
    pub calls: Option<Vec<CallExtract>>,
    pub show_calls: bool,
    pub show_logs: bool,
    pub opcodes: Option<Vec<MEVOpcode>>,
    pub show_opcodes: bool,
    pub state_diff: Option<MEVStateDiff>,
    pub show_state_diff: bool,
}

// Parquet row:
// block_number 0
// transaction_index 1
// transaction_hash 2
// nonce 3
// from_address 4
// to_address 5
// value_binary 6
// value_string 7
// value_f64 8
// input 9
// gas_limit 10
// gas_used 11
// gas_price 12
// transaction_type 13
// max_priority_fee_per_gas 14
// max_fee_per_gas 15
// success 16
// n_input_bytes 17
// n_input_zero_bytes 18
// n_input_nonzero_bytes 19
// chain_id 20
#[hotpath::measure_all(future = true)]
impl MEVTransaction {
    pub async fn tx_data_from_parquet_row(
        batch: &arrow::record_batch::RecordBatch,
        row_idx: usize,
    ) -> Result<(TxData, u64)> {
        let get_string_value =
            |col_idx: usize| -> String { get_parquet_string_value(batch, col_idx, row_idx) };

        let block_number = get_string_value(0).parse::<u64>().unwrap();

        let to_address_str = get_string_value(5);
        let to_address = if to_address_str == "0x" || to_address_str.is_empty() {
            TxKind::Create
        } else {
            TxKind::Call(Address::from_str(&to_address_str).unwrap())
        };

        let tx_hash_str = get_string_value(2);
        let tx_hash = FixedBytes::from_str(&tx_hash_str).unwrap();

        let inner = TransactionRequest {
            from: Some(Address::from_str(&get_string_value(4)).unwrap()),
            to: Some(to_address),
            input: TransactionInput::new(Bytes::from_str(&get_string_value(9)).unwrap()),
            gas_price: Some(get_string_value(12).parse::<u128>().unwrap()),
            gas: Some(get_string_value(10).parse::<u64>().unwrap()),
            value: Some(U256::from_str(&get_string_value(7)).unwrap()),
            nonce: Some(get_string_value(3).parse::<u64>().unwrap()),
            chain_id: Some(get_string_value(20).parse::<u64>().unwrap()),
            max_fee_per_gas: Some(get_string_value(15).parse::<u128>().unwrap_or(0)),
            max_priority_fee_per_gas: Some(get_string_value(14).parse::<u128>().unwrap_or(0)),
            access_list: Some(AccessList::from(vec![])),
            ..Default::default()
        };

        Ok((
            TxData {
                req: inner,
                tx_hash,
                receipt: ReceiptData {
                    success: get_string_value(16).parse::<bool>().unwrap(),
                    effective_gas_price: get_string_value(12).parse::<u128>().unwrap(),
                    gas_used: get_string_value(11).parse::<u64>().unwrap(),
                },
            },
            block_number,
        ))
    }

    pub async fn new(
        native_token_price: Option<f64>,
        chain: Arc<EVMChain>,
        tx_req: &TransactionRequest,
        block_number: u64,
        receipt_data: ReceiptData,
        tx_hash: FixedBytes<32>,
        index: u64,
        sqlite: &SqlitePool,
        ens_lookup: &ENSLookup,
        provider: &Arc<GenericProvider>,
        top_metadata: bool,
        show_calls: bool,
        show_logs: bool,
        show_opcodes: bool,
        show_state_diff: bool,
    ) -> Result<Self> {
        let (signature_hash, signature) =
            extract_signature(tx_req.input.input.as_ref(), index, tx_req.to, sqlite).await?;

        let mev_address =
            MEVAddress::new(tx_req.from.expect("TX from missing"), ens_lookup, provider).await?;

        let to_kind = tx_req.to.unwrap_or(TxKind::Create);
        let target = match to_kind {
            TxKind::Call(address) => Some(MEVAddress::new(address, ens_lookup, provider).await?),
            TxKind::Create => None,
        };

        Ok(Self {
            block_number,
            native_token_price,
            chain,
            nonce: tx_req.nonce.unwrap_or(0),
            tx_hash,
            index,
            log_groups: vec![],
            signature,
            signature_hash,
            source: mev_address,
            target,
            to: to_kind,
            inner: tx_req.clone(),
            coinbase_transfer: None,
            receipt: receipt_data,
            top_metadata,
            calls: None,
            show_calls,
            show_logs,
            opcodes: None,
            show_opcodes,
            state_diff: None,
            show_state_diff,
        })
    }

    pub fn add_log(&mut self, new_log: MEVLog) {
        match self.log_groups.last() {
            Some(last_log) if last_log.source() == new_log.source() => {
                self.log_groups.last_mut().unwrap().add_log(new_log);
            }
            Some(_) => {
                self.log_groups.push(MEVLogGroup::new(
                    new_log.source(),
                    vec![new_log],
                    self.chain.clone(),
                ));
            }
            None => {
                self.log_groups.push(MEVLogGroup::new(
                    new_log.source(),
                    vec![new_log],
                    self.chain.clone(),
                ));
            }
        }
    }

    pub fn from_ens_name(&self) -> Option<&str> {
        self.source.ens_name()
    }

    pub fn to_ens_name(&self) -> Option<&str> {
        self.target.as_ref().and_then(|t| t.ens_name())
    }

    pub fn from(&self) -> Address {
        self.source.address()
    }

    pub fn to(&self) -> Option<Address> {
        match self.to {
            TxKind::Call(address) => Some(address),
            TxKind::Create => None,
        }
    }

    pub fn logs(&self) -> Vec<&MEVLog> {
        self.log_groups
            .iter()
            .flat_map(|group| &group.logs)
            .collect()
    }

    pub fn log_groups(&self) -> &Vec<MEVLogGroup> {
        &self.log_groups
    }

    pub fn gas_tx_cost(&self) -> u128 {
        // TODO chandle tx pos 0 on OP chains, some receipt info missing
        self.receipt.gas_used as u128 * self.receipt.effective_gas_price
    }

    pub fn full_tx_cost(&self) -> Option<U256> {
        self.coinbase_transfer.map(|coinbase_transfer| {
            U256::from(self.receipt.gas_used as u128 * self.receipt.effective_gas_price)
                .add(coinbase_transfer)
        })
    }

    pub fn effective_gas_price(&self) -> U256 {
        U256::from(self.receipt.effective_gas_price)
    }

    pub fn full_effective_gas_price(&self) -> U256 {
        if self.receipt.gas_used == 0 {
            U256::from(0)
        } else {
            self.full_tx_cost()
                .expect("must be traced")
                .div(U256::from(self.receipt.gas_used))
        }
    }

    pub fn value(&self) -> U256 {
        self.inner.value.unwrap_or(U256::ZERO)
    }
}

#[hotpath::measure(log = true, future = true)]
pub async fn extract_signature(
    input: Option<&Bytes>,
    index: u64,
    to: Option<TxKind>,
    sqlite: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(Option<String>, String), eyre::Error> {
    if to == Some(TxKind::Create) {
        return Ok((None, "CREATE()".to_string()));
    }

    let signature_hash = input
        .filter(|i| i.len() >= 4)
        .map(|i| format!("0x{}", hex::encode(&i[..4])));
    let signature = match signature_hash.clone() {
        Some(sig) => {
            if let Some(sig_overwrite) = find_sig_overwrite(&sig, index) {
                sig_overwrite.clone()
            } else {
                let sig_str = DBMethod::find_by_hash(&sig, sqlite).await?;
                sig_str.unwrap_or(UNKNOWN.to_string())
            }
        }
        None => ETH_TRANSFER.to_string(),
    };
    Ok((signature_hash, signature))
}

pub fn calculate_create_address(nonce: u64, from: Address) -> Address {
    let mut out = Vec::new();
    let list: [&dyn Encodable; 2] = [&from, &U256::from(nonce)];
    alloy::rlp::encode_list::<_, dyn Encodable>(&list, &mut out);
    let keccak = keccak256(&out);
    Address::from_slice(&keccak[12..])
}

pub fn eth_to_usd(value: U256, token_price: f64) -> f64 {
    let decimals = 18;
    let value_dec = BigDecimal::from_str(&value.to_string()).unwrap();
    let one_eth_dec = BigDecimal::from_str(&format!("1e{decimals}")).unwrap();
    let price_dec = BigDecimal::from_str(&token_price.to_string()).unwrap();

    let result = (value_dec / one_eth_dec) * price_dec;
    let result_rounded = result.round(4);

    result_rounded.to_f64().unwrap_or(0.0)
}

pub fn display_usd(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    let formatted = format!("{rounded:.2}");
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = parts.get(1).unwrap_or(&"00");

    // Add commas to integer part
    let mut result = String::new();
    let chars: Vec<char> = integer_part.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(*ch);
    }

    format!("${result}.{decimal_part}")
}

pub fn display_token(value: U256, currency_symbol: &str, approx: bool) -> String {
    if value == U256::ZERO {
        return format!("0 {currency_symbol}");
    }
    let prefix = if approx { "~" } else { "" };
    format!("{}{:.5} {}", prefix, wei_to_eth(value), currency_symbol)
}

pub fn display_token_and_usd(
    value: U256,
    token_price: Option<f64>,
    currency_symbol: &str,
) -> String {
    if token_price.is_none() {
        return display_token(value, currency_symbol, false);
    }

    let token_price = token_price.unwrap();
    let usd_value = eth_to_usd(value, token_price);

    if value == U256::ZERO {
        return display_token(value, currency_symbol, false);
    }

    let token_display = if usd_value < 0.01 {
        display_token(value, currency_symbol, true)
    } else {
        display_token(value, currency_symbol, false)
    };

    let usd_display = if usd_value < 0.01 {
        format!("~{}", display_usd(usd_value))
    } else {
        display_usd(usd_value)
    };

    format!("{token_display} | {usd_display}")
}

// Common signatures, that are duplicate and mismatched in the database
pub fn find_sig_overwrite(signature: &str, tx_index: u64) -> Option<String> {
    if signature == "0x098999be" && tx_index == 0 {
        return Some("setL1BlockValuesIsthmus()".to_string());
    }
    None
}
