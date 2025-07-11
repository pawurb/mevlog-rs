use std::{
    fmt,
    ops::{Add, Div},
    str::FromStr,
    sync::Arc,
};

use alloy::{
    rlp::Encodable,
    rpc::types::{TransactionInput, TransactionRequest},
};
use bigdecimal::{BigDecimal, ToPrimitive};
use colored::Colorize;
use eyre::Result;
use revm::primitives::{keccak256, AccessList, Address, Bytes, FixedBytes, TxKind, U256};
use sqlx::SqlitePool;

use super::{
    db_method::DBMethod, mev_address::MEVAddress, mev_block::TxData, mev_log::MEVLog,
    mev_log_group::MEVLogGroup,
};
use crate::{
    misc::{
        ens_utils::ENSLookup,
        utils::{wei_to_eth, ETH_TRANSFER, GWEI, GWEI_F64, SEPARATOR, UNKNOWN},
    },
    models::evm_chain::EVMChain,
    GenericProvider,
};

const LABEL_WIDTH: usize = 18;

#[derive(Debug, Clone)]
pub struct ReceiptData {
    pub success: bool,
    pub effective_gas_price: u128,
    pub gas_used: u64,
}

#[derive(Debug, Clone)]
pub struct CallExtract {
    pub from: Address,
    pub to: Address,
    pub signature: String,
    pub signature_hash: Option<String>,
}

impl fmt::Display for CallExtract {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} -> {}::{} ({})",
            format!("{}", self.from).yellow(),
            format!("{}", self.to).green(),
            self.signature.purple(),
            self.signature_hash
                .as_ref()
                .unwrap_or(&"no signature found".to_string())
        )?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct MEVTransaction {
    native_token_price: f64,
    pub chain: EVMChain,
    pub signature: String,
    pub signature_hash: Option<String>,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    pub inner: TransactionRequest,
    log_groups: Vec<MEVLogGroup>,
    source: MEVAddress,
    to: TxKind,
    pub nonce: u64,
    pub coinbase_transfer: Option<U256>,
    pub receipt: ReceiptData,
    pub top_metadata: bool,
    pub calls: Option<Vec<CallExtract>>,
    pub show_calls: bool,
}

// CSV row:
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
impl MEVTransaction {
    #[allow(clippy::too_many_arguments)]
    pub async fn req_from_csv(record: csv::StringRecord) -> Result<TxData> {
        let to = if record[5].to_string() == "0x" || record[5].is_empty() {
            TxKind::Create
        } else {
            TxKind::Call(Address::from_str(&record[5]).unwrap())
        };

        let tx_hash = FixedBytes::from_str(&record[2]).unwrap();

        let inner = TransactionRequest {
            from: Some(Address::from_str(&record[4]).unwrap()),
            to: Some(to),
            input: TransactionInput::new(Bytes::from_str(&record[9]).unwrap()),
            gas_price: Some(record[11].to_string().parse::<u128>().unwrap()),
            gas: Some(record[10].to_string().parse::<u64>().unwrap()),
            value: Some(U256::from_str(&record[7]).unwrap()),
            nonce: Some(record[3].to_string().parse::<u64>().unwrap()),
            chain_id: Some(record[12].to_string().parse::<u64>().unwrap()),
            max_fee_per_gas: Some(record[15].to_string().parse::<u128>().unwrap_or(0)),
            max_priority_fee_per_gas: Some(record[14].to_string().parse::<u128>().unwrap_or(0)),
            access_list: Some(AccessList::from(vec![])),
            ..Default::default()
        };

        Ok(TxData {
            req: inner,
            tx_hash,
            receipt: ReceiptData {
                success: record[16].to_string().parse::<bool>().unwrap(),
                effective_gas_price: record[12].to_string().parse::<u128>().unwrap(),
                gas_used: record[11].to_string().parse::<u64>().unwrap(),
            },
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        native_token_price: f64,
        chain: EVMChain,
        tx_req: TransactionRequest,
        receipt_data: ReceiptData,
        tx_hash: FixedBytes<32>,
        index: u64,
        sqlite: &SqlitePool,
        ens_lookup: &ENSLookup,
        provider: &Arc<GenericProvider>,
        top_metadata: bool,
        show_calls: bool,
    ) -> Result<Self> {
        let (signature_hash, signature) =
            extract_signature(&chain, tx_req.input.input.as_ref(), index, sqlite).await?;

        let mev_address =
            MEVAddress::new(tx_req.from.expect("TX from missing"), ens_lookup, provider).await?;

        Ok(Self {
            native_token_price,
            chain,
            nonce: tx_req.nonce.unwrap_or(0),
            tx_hash,
            index,
            log_groups: vec![],
            signature,
            signature_hash,
            source: mev_address,
            to: tx_req.to.unwrap_or(TxKind::Create),
            inner: tx_req,
            coinbase_transfer: None,
            receipt: receipt_data,
            top_metadata,
            calls: None,
            show_calls,
        })
    }

    pub fn add_log(&mut self, new_log: MEVLog) {
        match self.log_groups.last() {
            Some(last_log) => {
                if last_log.source() == new_log.source() {
                    self.log_groups.last_mut().unwrap().add_log(new_log);
                } else {
                    self.log_groups.push(MEVLogGroup::new(
                        new_log.source(),
                        vec![new_log],
                        self.chain.clone(),
                    ));
                }
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

    pub fn ens_name(&self) -> Option<&str> {
        self.source.ens_name()
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
        let mut logs = vec![];
        for log_group in &self.log_groups {
            for log in &log_group.logs {
                logs.push(log);
            }
        }

        logs
    }

    pub fn gas_tx_cost(&self) -> u128 {
        // TODO chandle tx pos 0 on OP chains, some receipt info missing
        self.receipt.gas_used as u128 * self.receipt.effective_gas_price
    }

    pub fn full_tx_cost(&self) -> U256 {
        U256::from(self.receipt.gas_used as u128 * self.receipt.effective_gas_price)
            .add(self.coinbase_transfer.expect("must be traced"))
    }

    pub fn effective_gas_price(&self) -> U256 {
        U256::from(self.receipt.effective_gas_price)
    }

    pub fn full_effective_gas_price(&self) -> U256 {
        if self.receipt.gas_used == 0 {
            U256::from(0)
        } else {
            self.full_tx_cost().div(U256::from(self.receipt.gas_used))
        }
    }

    pub fn value(&self) -> U256 {
        self.inner.value.unwrap_or(U256::ZERO)
    }
}

pub async fn extract_signature(
    chain: &EVMChain,
    input: Option<&Bytes>,
    index: u64,
    sqlite: &sqlx::Pool<sqlx::Sqlite>,
) -> Result<(Option<String>, String), eyre::Error> {
    let signature_hash = {
        if let Some(input) = input {
            if input.len() >= 4 {
                let hash = format!("0x{}", hex::encode(&input[..4]));
                Some(hash)
            } else {
                None
            }
        } else {
            None
        }
    };
    let signature = match signature_hash.clone() {
        Some(sig) => {
            if let Some(override_sig) = chain.signature_overrides().get(&(sig.clone(), index)) {
                override_sig.clone()
            } else {
                let sig_str = DBMethod::find_by_hash(&sig, sqlite).await?;
                sig_str.unwrap_or(UNKNOWN.to_string())
            }
        }
        None => ETH_TRANSFER.to_string(),
    };
    Ok((signature_hash, signature))
}

impl fmt::Display for MEVTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.top_metadata {
            for log in &self.log_groups {
                write!(f, "{log}")?;
            }
        }

        if self.top_metadata {
            writeln!(f, "{SEPARATOR}")?;
            writeln!(
                f,
                "[{}] {}",
                self.index,
                &format!("{}/tx/{}", self.chain.explorer_url, self.tx_hash).yellow(),
            )?;

            writeln!(f)?;
            writeln!(f, "{} ->", self.source)?;
            writeln!(f, "  {}", display_target(self))?;
        } else {
            writeln!(f, "{} ->", self.source)?;
            writeln!(f, "  {}", display_target(self))?;

            writeln!(
                f,
                "[{}] {}",
                self.index,
                &format!("{}/tx/{}", self.chain.explorer_url, self.tx_hash).yellow(),
            )?;
        }

        writeln!(f)?;

        if !self.receipt.success {
            writeln!(f, "{}", "Tx reverted!".red().bold())?;
        }

        if self.show_calls {
            if let Some(calls) = &self.calls {
                writeln!(f, "{SEPARATOR}")?;
                writeln!(f, "Calls:")?;
                for call in calls {
                    writeln!(f, "{call}")?;
                }
                writeln!(f, "{SEPARATOR}")?;
            }
        }

        writeln!(
            f,
            "{:width$} {}",
            "Value:".green().bold(),
            display_token_and_usd(
                self.value(),
                self.native_token_price,
                &self.chain.currency_symbol
            ),
            width = LABEL_WIDTH
        )?;

        writeln!(
            f,
            "{:width$} {:.2} GWEI",
            "Gas Price:".green().bold(),
            self.receipt.effective_gas_price as f64 / GWEI_F64,
            width = LABEL_WIDTH
        )?;

        writeln!(
            f,
            "{:width$} {}",
            "Gas Tx Cost:".green().bold(),
            display_token_and_usd(
                U256::from(self.gas_tx_cost()),
                self.native_token_price,
                &self.chain.currency_symbol
            ),
            width = LABEL_WIDTH
        )?;

        match self.coinbase_transfer {
            Some(coinbase_transfer) => {
                writeln!(
                    f,
                    "{:width$} {}",
                    "Coinbase Transfer:".green().bold(),
                    display_token_and_usd(
                        coinbase_transfer,
                        self.native_token_price,
                        &self.chain.currency_symbol
                    ),
                    width = LABEL_WIDTH
                )?;

                writeln!(
                    f,
                    "{:width$} {}",
                    "Real Tx Cost:".green().bold(),
                    display_token_and_usd(
                        self.full_tx_cost(),
                        self.native_token_price,
                        &self.chain.currency_symbol
                    ),
                    width = LABEL_WIDTH
                )?;

                writeln!(
                    f,
                    "{:width$} {:.2} GWEI",
                    "Real Gas Price:".green().bold(),
                    self.full_effective_gas_price()
                        .div(GWEI.div(U256::from(100)))
                        .to_string()
                        .parse::<f64>()
                        .unwrap()
                        / 100.0,
                    width = LABEL_WIDTH
                )?;
            }
            None => {
                writeln!(
                    f,
                    "{:width$} {}",
                    "Coinbase Transfer:".yellow().bold(),
                    "N/A".yellow().bold(),
                    width = LABEL_WIDTH
                )?;
                writeln!(
                    f,
                    "{:width$} {}",
                    "Real Tx Cost:".yellow().bold(),
                    "N/A".yellow().bold(),
                    width = LABEL_WIDTH
                )?;
                writeln!(
                    f,
                    "{:width$} {}",
                    "Real Gas Price:".yellow().bold(),
                    "N/A".yellow().bold(),
                    width = LABEL_WIDTH
                )?;
            }
        }

        if self.top_metadata {
            if !&self.log_groups.is_empty() {
                writeln!(f)?;
            }

            for log in &self.log_groups {
                write!(f, "{log}")?;
            }
        }

        if !self.top_metadata {
            writeln!(f, "{SEPARATOR}")?;
        }

        Ok(())
    }
}

fn display_target(tx: &MEVTransaction) -> String {
    match tx.to {
        TxKind::Create => {
            if let Some(from) = tx.inner.from {
                let mut out = Vec::new();

                let list: [&dyn Encodable; 2] = [&from, &U256::from(tx.nonce)];

                alloy::rlp::encode_list::<_, dyn Encodable>(&list, &mut out);

                let keccak = keccak256(&out);
                let contract_address = &keccak[12..];
                let contract_address_str = format!("0x{}", hex::encode(contract_address));

                format!("{}{}", "CREATE::".green(), contract_address_str.red(),)
            } else {
                format!("{}", "CREATE()".green())
            }
        }
        TxKind::Call(address) => {
            format!("{}::{}", address.to_string().green(), tx.signature.purple())
        }
    }
}

fn eth_to_usd(value: U256, token_price: f64) -> f64 {
    let decimals = 18;
    let value_dec = BigDecimal::from_str(&value.to_string()).unwrap();
    let one_eth_dec = BigDecimal::from_str(&format!("1e{decimals}")).unwrap();
    let price_dec = BigDecimal::from_str(&token_price.to_string()).unwrap();

    let result = (value_dec / one_eth_dec) * price_dec;
    let result_rounded = result.round(4);

    result_rounded.to_f64().unwrap_or(0.0)
}

fn display_usd(value: f64) -> String {
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

fn display_token_and_usd(value: U256, token_price: f64, currency_symbol: &str) -> String {
    let usd_value = eth_to_usd(value, token_price);

    if value == U256::ZERO {
        return format!("0 {currency_symbol}");
    }

    if usd_value < 0.01 {
        format!(
            "~{:.5} {} | ~{}",
            wei_to_eth(value),
            currency_symbol,
            display_usd(usd_value)
        )
    } else {
        format!(
            "{:.5} {} | {}",
            wei_to_eth(value),
            currency_symbol,
            display_usd(usd_value)
        )
    }
}
