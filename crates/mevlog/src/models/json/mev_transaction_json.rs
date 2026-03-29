use revm::primitives::{Address, FixedBytes, TxKind, U256};
use serde::{Deserialize, Serialize, Serializer, ser::SerializeStruct};

use crate::{
    misc::utils::ToU128,
    models::{
        json::{
            mev_log_group_json::MEVLogGroupJson, mev_opcode_json::MEVOpcodeJson,
            mev_state_diff_json::MEVStateDiffJson,
        },
        mev_transaction::{
            CallExtract, MEVTransaction, calculate_create_address, display_token,
            display_token_and_usd, display_usd, eth_to_usd,
        },
    },
};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MEVTransactionJson {
    pub block_number: u64,
    pub signature: String,
    pub signature_hash: Option<String>,
    pub tx_hash: FixedBytes<32>,
    pub index: u64,
    pub from: Address,
    pub from_ens: Option<String>,
    pub to: Option<Address>,
    pub to_ens: Option<String>,
    pub nonce: u64,
    pub value: String,
    pub display_value: String,
    pub coinbase_transfer: Option<String>,
    pub display_coinbase_transfer: Option<String>,
    pub display_coinbase_transfer_usd: Option<String>,
    pub success: bool,
    pub gas_price: u128,
    pub gas_used: u64,
    pub tx_cost: u128,
    pub display_tx_cost: String,
    pub display_tx_cost_usd: Option<String>,
    pub full_tx_cost: Option<u128>,
    pub display_full_tx_cost: Option<String>,
    pub display_full_tx_cost_usd: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evm_calls: Vec<CallExtract>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<MEVLogGroupJson>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evm_opcodes: Vec<MEVOpcodeJson>,
    #[serde(default, skip_serializing_if = "MEVStateDiffJson::is_empty")]
    pub evm_state_diff: MEVStateDiffJson,
}

impl From<&MEVTransaction> for MEVTransactionJson {
    fn from(tx: &MEVTransaction) -> Self {
        let logs = tx.log_groups().iter().map(MEVLogGroupJson::from).collect();

        let gas_tx_cost = tx.receipt.gas_used as u128 * tx.receipt.effective_gas_price;
        let full_tx_cost = tx.full_tx_cost().map(|amt| amt.to_u128());

        let to = match tx.to {
            TxKind::Create => Some(calculate_create_address(tx.nonce, tx.from())),
            TxKind::Call(address) => Some(address),
        };

        Self {
            block_number: tx.block_number,
            signature: tx.signature.clone(),
            signature_hash: tx.signature_hash.clone(),
            tx_hash: tx.tx_hash,
            index: tx.index,
            from: tx.from(),
            from_ens: tx.from_ens_name().map(|s| s.to_string()),
            to,
            to_ens: tx.to_ens_name().map(|s| s.to_string()),
            nonce: tx.nonce,
            value: tx.value().to_string(),
            coinbase_transfer: tx.coinbase_transfer.map(|amt| amt.to_string()),
            display_coinbase_transfer: tx
                .coinbase_transfer
                .map(|amt| display_token(amt, &tx.chain.currency_symbol, false)),
            display_coinbase_transfer_usd: tx.coinbase_transfer.and_then(|amt| {
                tx.native_token_price
                    .map(|price| display_usd(eth_to_usd(amt, price)))
            }),
            success: tx.receipt.success,
            gas_price: tx.receipt.effective_gas_price,
            tx_cost: gas_tx_cost,
            display_tx_cost: display_token(
                U256::from(gas_tx_cost),
                &tx.chain.currency_symbol,
                false,
            ),
            display_tx_cost_usd: tx
                .native_token_price
                .map(|price| display_usd(eth_to_usd(U256::from(gas_tx_cost), price))),
            display_value: display_token_and_usd(
                tx.value(),
                tx.native_token_price,
                &tx.chain.currency_symbol,
            ),
            full_tx_cost,
            display_full_tx_cost: full_tx_cost
                .map(|amt| display_token(U256::from(amt), &tx.chain.currency_symbol, false)),
            display_full_tx_cost_usd: full_tx_cost.and_then(|amt| {
                tx.native_token_price
                    .map(|price| display_usd(eth_to_usd(U256::from(amt), price)))
            }),
            gas_used: tx.receipt.gas_used,
            evm_calls: tx.calls.clone().unwrap_or_default(),
            logs,
            evm_opcodes: tx
                .opcodes
                .as_ref()
                .map(|ops| ops.iter().map(MEVOpcodeJson::from).collect())
                .unwrap_or_default(),
            evm_state_diff: tx
                .state_diff
                .as_ref()
                .map(MEVStateDiffJson::from)
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Copy)]
pub struct JsonSerializeOpts {
    pub include_logs: bool,
    pub include_evm_calls: bool,
    pub include_evm_opcodes: bool,
    pub include_evm_state_diff: bool,
}

struct MEVTransactionJsonOutput<'a> {
    transaction: &'a MEVTransactionJson,
    opts: JsonSerializeOpts,
}

impl Serialize for MEVTransactionJsonOutput<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tx = self.transaction;
        let mut output = serializer.serialize_struct("MEVTransactionJson", 22)?;

        output.serialize_field("block_number", &tx.block_number)?;
        output.serialize_field("signature", &tx.signature)?;
        output.serialize_field("signature_hash", &tx.signature_hash)?;
        output.serialize_field("tx_hash", &tx.tx_hash)?;
        output.serialize_field("index", &tx.index)?;
        output.serialize_field("from", &tx.from)?;
        output.serialize_field("from_ens", &tx.from_ens)?;
        output.serialize_field("to", &tx.to)?;
        output.serialize_field("to_ens", &tx.to_ens)?;
        output.serialize_field("nonce", &tx.nonce)?;
        output.serialize_field("value", &tx.value)?;
        output.serialize_field("display_value", &tx.display_value)?;
        output.serialize_field("coinbase_transfer", &tx.coinbase_transfer)?;
        output.serialize_field("display_coinbase_transfer", &tx.display_coinbase_transfer)?;
        output.serialize_field(
            "display_coinbase_transfer_usd",
            &tx.display_coinbase_transfer_usd,
        )?;
        output.serialize_field("success", &tx.success)?;
        output.serialize_field("gas_price", &tx.gas_price)?;
        output.serialize_field("gas_used", &tx.gas_used)?;
        output.serialize_field("tx_cost", &tx.tx_cost)?;
        output.serialize_field("display_tx_cost", &tx.display_tx_cost)?;
        output.serialize_field("display_tx_cost_usd", &tx.display_tx_cost_usd)?;
        output.serialize_field("full_tx_cost", &tx.full_tx_cost)?;
        output.serialize_field("display_full_tx_cost", &tx.display_full_tx_cost)?;
        output.serialize_field("display_full_tx_cost_usd", &tx.display_full_tx_cost_usd)?;
        if self.opts.include_evm_calls && !tx.evm_calls.is_empty() {
            output.serialize_field("evm_calls", &tx.evm_calls)?;
        }

        if self.opts.include_logs && !tx.logs.is_empty() {
            output.serialize_field("logs", &tx.logs)?;
        }

        if self.opts.include_evm_opcodes && !tx.evm_opcodes.is_empty() {
            output.serialize_field("evm_opcodes", &tx.evm_opcodes)?;
        }
        if self.opts.include_evm_state_diff && !tx.evm_state_diff.is_empty() {
            output.serialize_field("evm_state_diff", &tx.evm_state_diff)?;
        }
        output.end()
    }
}

pub fn serialize_transactions_json(
    transactions: &[MEVTransactionJson],
    opts: JsonSerializeOpts,
    pretty: bool,
) -> serde_json::Result<String> {
    let output: Vec<_> = transactions
        .iter()
        .map(|transaction| MEVTransactionJsonOutput { transaction, opts })
        .collect();

    if pretty {
        serde_json::to_string_pretty(&output)
    } else {
        serde_json::to_string(&output)
    }
}

#[cfg(test)]
mod tests {
    use crate::models::json::mev_log_group_json::MEVLogGroupJson;
    use crate::models::json::mev_log_json::MEVLogJson;

    use super::*;

    fn base_fields() -> Vec<&'static str> {
        vec![
            "block_number",
            "signature",
            "signature_hash",
            "tx_hash",
            "index",
            "from",
            "from_ens",
            "to",
            "to_ens",
            "nonce",
            "value",
            "display_value",
            "coinbase_transfer",
            "display_coinbase_transfer",
            "display_coinbase_transfer_usd",
            "success",
            "gas_price",
            "gas_used",
            "tx_cost",
            "display_tx_cost",
            "display_tx_cost_usd",
            "full_tx_cost",
            "display_full_tx_cost",
            "display_full_tx_cost_usd",
        ]
    }

    fn make_tx(with_logs: bool) -> MEVTransactionJson {
        let logs = if with_logs {
            vec![MEVLogGroupJson {
                source: Address::ZERO,
                logs: vec![MEVLogJson {
                    source: Address::ZERO,
                    signature: "Transfer(address,address,uint256)".to_string(),
                    symbol: None,
                    amount: None,
                    topics: vec![],
                    data: "00".to_string(),
                }],
            }]
        } else {
            vec![]
        };

        MEVTransactionJson {
            block_number: 1,
            signature: "test()".to_string(),
            signature_hash: None,
            tx_hash: FixedBytes::ZERO,
            index: 0,
            from: Address::ZERO,
            from_ens: None,
            to: Some(Address::ZERO),
            to_ens: None,
            nonce: 0,
            value: "0".to_string(),
            display_value: "0 ETH".to_string(),
            coinbase_transfer: None,
            display_coinbase_transfer: None,
            display_coinbase_transfer_usd: None,
            success: true,
            gas_price: 0,
            gas_used: 0,
            tx_cost: 0,
            display_tx_cost: "0 ETH".to_string(),
            display_tx_cost_usd: None,
            full_tx_cost: None,
            display_full_tx_cost: None,
            display_full_tx_cost_usd: None,
            evm_calls: vec![],
            logs,
            evm_opcodes: vec![],
            evm_state_diff: MEVStateDiffJson::default(),
        }
    }

    fn get_json_keys(json: &str) -> Vec<String> {
        let arr: Vec<serde_json::Value> = serde_json::from_str(json).unwrap();
        let obj = arr[0].as_object().unwrap();
        obj.keys().cloned().collect()
    }

    fn opts_none() -> JsonSerializeOpts {
        JsonSerializeOpts {
            include_logs: false,
            include_evm_calls: false,
            include_evm_opcodes: false,
            include_evm_state_diff: false,
        }
    }

    fn opts_with_logs() -> JsonSerializeOpts {
        JsonSerializeOpts {
            include_logs: true,
            ..opts_none()
        }
    }

    #[test]
    fn test_include_logs_false_omits_log_groups() {
        let tx = make_tx(true);
        let json = serialize_transactions_json(&[tx], opts_none(), false).unwrap();
        let keys = get_json_keys(&json);

        let expected = base_fields();
        assert_eq!(keys.len(), expected.len());
        for field in &expected {
            assert!(keys.contains(&field.to_string()), "missing field: {field}");
        }
        assert!(!keys.contains(&"logs".to_string()));
    }

    #[test]
    fn test_include_logs_true_includes_log_groups() {
        let tx = make_tx(true);
        let json = serialize_transactions_json(&[tx], opts_with_logs(), false).unwrap();
        let keys = get_json_keys(&json);

        let mut expected = base_fields();
        expected.push("logs");
        assert_eq!(keys.len(), expected.len());
        for field in &expected {
            assert!(keys.contains(&field.to_string()), "missing field: {field}");
        }
    }

    #[test]
    fn test_include_logs_true_empty_logs_omits_log_groups() {
        let tx = make_tx(false);
        let json = serialize_transactions_json(&[tx], opts_with_logs(), false).unwrap();
        let keys = get_json_keys(&json);

        let expected = base_fields();
        assert_eq!(keys.len(), expected.len());
        assert!(!keys.contains(&"logs".to_string()));
    }
}
