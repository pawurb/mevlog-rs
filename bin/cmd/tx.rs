use std::collections::HashSet;

use alloy::providers::Provider;
use eyre::{Result, eyre};
use mevlog::{
    misc::{
        args_parsing::PositionRange,
        ens_utils::ENSLookup,
        shared_init::{ConnOpts, OutputFormat, SharedOpts, init_deps},
        symbol_utils::ERC20SymbolsLookup,
        utils::get_native_token_price,
    },
    models::{mev_block::generate_block, txs_filter::TxsFilter},
};
use revm::primitives::FixedBytes;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinimalTxInfo {
    block_number: Option<String>,
    transaction_index: Option<String>,
}

#[derive(Debug, clap::Parser)]
pub struct TxArgs {
    tx_hash: FixedBytes<32>,
    #[arg(
        long,
        short = 'b',
        help = "'before' means newer transactions (smaller indexes)"
    )]
    before: Option<u8>,
    #[arg(
        long,
        short = 'a',
        help = "'after' means older transactions (larger indexes)"
    )]
    after: Option<u8>,

    #[arg(short, long, alias = "r", help = "Reverse the order of txs")]
    pub reverse: bool,

    #[arg(
        long,
        alias = "tm",
        help = "Display block and txs metadata info on top"
    )]
    pub top_metadata: bool,

    #[arg(long, help = "Display EVM opcodes executed by the transaction")]
    pub ops: bool,

    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl TxArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        check_range(self.before, "--before")?;
        check_range(self.after, "--after")?;

        if self.shared_opts.show_calls && self.shared_opts.trace.is_none() {
            eyre::bail!("'--show-calls' is supported only with --trace [rpc|revm] enabled")
        }

        if self.ops && self.shared_opts.trace.is_none() {
            eyre::bail!("'--ops' is supported only with --trace [rpc|revm] enabled")
        }

        let deps = init_deps(&self.conn_opts).await?;

        let tx_info: MinimalTxInfo = deps
            .provider
            .client()
            .request("eth_getTransactionByHash", (self.tx_hash,))
            .await?;

        let block_number = tx_info
            .block_number
            .ok_or_else(|| eyre!("transaction not found or not mined"))?;
        let tx_index = tx_info
            .transaction_index
            .ok_or_else(|| eyre!("transaction index not found"))?;

        let (block_number, tx_index) = (
            u64::from_str_radix(block_number.trim_start_matches("0x"), 16)?,
            u64::from_str_radix(tx_index.trim_start_matches("0x"), 16)?,
        );

        let tx_indexes = get_matching_indexes(tx_index, self.before, self.after);

        let max_index = tx_indexes
            .clone()
            .into_iter()
            .max()
            .expect("tx_indexes must have at least one element");

        let position_range = Some(PositionRange {
            from: 0,
            to: max_index,
        });

        let native_token_price = get_native_token_price(
            &deps.chain,
            &deps.provider,
            self.shared_opts.native_token_price,
        )
        .await?;

        let txs_filter = TxsFilter {
            tx_indexes: Some(tx_indexes),
            tx_from: None,
            tx_to: None,
            touching: None,
            tx_position: position_range,
            events: vec![],
            not_events: vec![],
            match_method: None,
            tx_cost: None,
            gas_price: None,
            real_tx_cost: None,
            real_gas_price: None,
            value: None,
            reversed_order: self.reverse,
            top_metadata: self.top_metadata,
            match_calls: vec![],
            show_calls: self.shared_opts.show_calls,
            failed: false,
            erc20_transfers: vec![],
            show_erc20_transfer_amount: self.shared_opts.erc20_transfer_amount,
            show_opcodes: self.ops,
        };

        let ens_lookup_mode = if deps.chain.is_mainnet() && self.shared_opts.ens {
            ENSLookup::Sync
        } else if deps.chain.is_mainnet() {
            ENSLookup::OnlyCached
        } else {
            ENSLookup::Disabled
        };

        let symbols_lookup = ERC20SymbolsLookup::lookup_mode(
            deps.symbols_lookup_worker,
            self.shared_opts.erc20_symbols,
        );

        let mev_block = generate_block(
            &deps.provider,
            &deps.sqlite,
            block_number,
            &ens_lookup_mode,
            &symbols_lookup,
            &txs_filter,
            &self.shared_opts,
            &deps.chain,
            &deps.rpc_url,
            native_token_price,
        )
        .await?;

        mev_block.print_with_format(&format);

        // Allow async ENS and symbols lookups to finish
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    }
}

fn get_matching_indexes(
    source_tx_index: u64,
    before: Option<u8>,
    after: Option<u8>,
) -> HashSet<u64> {
    let mut result = HashSet::new();
    result.insert(source_tx_index);

    if let Some(count) = after
        && count > 0
    {
        for i in 1..=count as u64 {
            result.insert(source_tx_index + i);
        }
    }

    if let Some(count) = before
        && count > 0
    {
        for i in 1..=count as u64 {
            if source_tx_index >= i {
                result.insert(source_tx_index - i);
            }
        }
    }

    result
}

fn check_range(value: Option<u8>, label: &str) -> Result<()> {
    if let Some(value) = value
        && value > 5
    {
        eyre::bail!("{} must be less than or equal 5", label);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_matching_indexes() {
        let expected1: HashSet<u64> = [7, 8, 9, 10, 11, 12].into_iter().collect();
        assert_eq!(get_matching_indexes(10, Some(3), Some(2)), expected1);

        let expected2: HashSet<u64> = [15].into_iter().collect();

        assert_eq!(get_matching_indexes(15, None, None), expected2);

        let expected3 = [0, 1, 2].into_iter().collect();
        assert_eq!(get_matching_indexes(0, Some(5), Some(2)), expected3);
    }
}
