use alloy::rpc::types::TransactionRequest;
use bigdecimal::{BigDecimal, ToPrimitive};
use colored::{ColoredString, Colorize};
use eyre::Result;
use revm::primitives::{Address, FixedBytes, TxKind, U256};
use sqlx::SqlitePool;
use std::ops::{Add, Div};
use std::str::FromStr;
use std::{fmt, sync::Arc};

use crate::misc::ens_utils::ENSLookup;
use crate::misc::utils::{
    wei_to_eth, ETHERSCAN_URL, ETH_TRANSFER, GWEI, GWEI_F64, SEPARATOR, UNKNOWN,
};
use crate::GenericProvider;

use super::{
    db_method::DBMethod, mev_address::MEVAddress, mev_log::MEVLog, mev_log_group::MEVLogGroup,
};

const LABEL_WIDTH: usize = 18;

#[derive(Debug)]
pub struct ReceiptData {
    pub success: bool,
    pub effective_gas_price: u128,
    pub gas_used: u64,
}

#[derive(Debug)]
pub struct MEVTransaction {
    eth_price: f64,
    pub method_name: String,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    log_groups: Vec<MEVLogGroup>,
    source: MEVAddress,
    to: TxKind,
    pub coinbase_transfer: Option<U256>,
    pub inner: TransactionRequest,
    pub receipt_data: Option<ReceiptData>,
}

impl MEVTransaction {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        eth_price: f64,
        tx_req: TransactionRequest,
        tx_hash: FixedBytes<32>,
        index: u64,
        sqlite: &Arc<SqlitePool>,
        ens_lookup: &ENSLookup,
        provider: &Arc<GenericProvider>,
    ) -> Result<Self> {
        let method_sig = if let Some(input) = tx_req.clone().input.input {
            if input.len() >= 8 {
                Some(format!("0x{}", hex::encode(&input[..4])))
            } else {
                None
            }
        } else {
            None
        };

        let signature = match method_sig.clone() {
            Some(sig) => {
                let sig = DBMethod::find_by_hash(&sig, sqlite).await?;
                sig.unwrap_or(UNKNOWN.to_string())
            }
            None => ETH_TRANSFER.to_string(),
        };

        let mev_address =
            MEVAddress::new(tx_req.from.expect("TX from missing"), ens_lookup, provider).await?;

        Ok(Self {
            eth_price,
            tx_hash,
            index,
            log_groups: vec![],
            method_name: signature,
            source: mev_address,
            to: tx_req.to.unwrap_or(TxKind::Create),
            coinbase_transfer: None,
            inner: tx_req,
            receipt_data: None,
        })
    }

    pub fn add_log(&mut self, new_log: MEVLog) {
        match self.log_groups.last() {
            Some(last_log) => {
                if last_log.source() == new_log.source() {
                    self.log_groups.last_mut().unwrap().add_log(new_log);
                } else {
                    self.log_groups
                        .push(MEVLogGroup::new(new_log.source(), vec![new_log]));
                }
            }
            None => {
                self.log_groups
                    .push(MEVLogGroup::new(new_log.source(), vec![new_log]));
            }
        }
    }

    pub fn ens_name(&self) -> Option<&str> {
        self.source.ens_name()
    }

    pub fn from(&self) -> Address {
        self.source.address()
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
        let receipt = self.receipt_data.as_ref().expect("must be fetched");
        receipt.gas_used as u128 * receipt.effective_gas_price
    }

    pub fn full_tx_cost(&self) -> U256 {
        let receipt = self.receipt_data.as_ref().expect("must be fetched");

        U256::from(receipt.gas_used as u128 * receipt.effective_gas_price)
            .add(self.coinbase_transfer.expect("must be traced"))
    }

    pub fn effective_gas_price(&self) -> U256 {
        let receipt = self.receipt_data.as_ref().expect("must be fetched");
        U256::from(receipt.effective_gas_price)
    }

    pub fn full_effective_gas_price(&self) -> U256 {
        let receipt = self.receipt_data.as_ref().expect("must be fetched");
        self.full_tx_cost().div(U256::from(receipt.gas_used))
    }
}

impl fmt::Display for MEVTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for log in &self.log_groups {
            write!(f, "{}", log)?;
        }
        writeln!(f, "{} ->", self.source)?;
        writeln!(
            f,
            "  {}::{}",
            display_target(self.to),
            self.method_name.purple(),
        )?;

        writeln!(
            f,
            "[{}] {}",
            self.index,
            &format!("{}/tx/{}", ETHERSCAN_URL, self.tx_hash).yellow(),
        )?;

        writeln!(f)?;

        if let Some(receipt) = &self.receipt_data {
            writeln!(
                f,
                "{:width$} {:.2} GWEI",
                "Gas Price:".green().bold(),
                receipt.effective_gas_price as f64 / GWEI_F64,
                width = LABEL_WIDTH
            )?;

            writeln!(
                f,
                "{:width$} {:.5} ETH | ${:.2}",
                "Gas Tx Cost:".green().bold(),
                wei_to_eth(U256::from(self.gas_tx_cost())),
                eth_to_usd(U256::from(self.gas_tx_cost()), self.eth_price),
                width = LABEL_WIDTH
            )?;
        }

        match self.coinbase_transfer {
            Some(coinbase_transfer) => {
                writeln!(
                    f,
                    "{:width$} {:.5} ETH | ${:.2}",
                    "Coinbase Transfer:".green().bold(),
                    wei_to_eth(coinbase_transfer),
                    eth_to_usd(coinbase_transfer, self.eth_price),
                    width = LABEL_WIDTH
                )?;

                writeln!(
                    f,
                    "{:width$} {:.5} ETH | ${:.2}",
                    "Real Tx Cost:".green().bold(),
                    wei_to_eth(self.full_tx_cost()),
                    eth_to_usd(self.full_tx_cost(), self.eth_price),
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
                writeln!(f, "{}", "[--trace disabled]".red().bold())?;
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

        writeln!(f, "{}", SEPARATOR)?;
        Ok(())
    }
}

fn display_target(to: TxKind) -> ColoredString {
    match to {
        TxKind::Create => "CREATE".green(),
        TxKind::Call(address) => format!("{}", address).green(),
    }
}

fn eth_to_usd(value: U256, token_price: f64) -> f64 {
    let decimals = 18;
    let value_dec = BigDecimal::from_str(&value.to_string()).unwrap();
    let one_eth_dec = BigDecimal::from_str(&format!("1e{}", decimals)).unwrap();
    let price_dec = BigDecimal::from_str(&token_price.to_string()).unwrap();

    let result = (value_dec / one_eth_dec) * price_dec;
    let result_rounded = result.round(4);

    result_rounded.to_f64().unwrap_or(0.0)
}
