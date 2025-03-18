use std::collections::HashSet;

use alloy::providers::Provider;
use eyre::{eyre, Result};
use mevlog::misc::ens_utils::ENSLookup;
use mevlog::misc::revm_tracing::init_revm_db;
use mevlog::misc::shared_init::{init_deps, ConnOpts, TraceMode};
use mevlog::misc::utils::SEPARATORER;
use mevlog::models::mev_block::MEVBlock;
use mevlog::models::txs_filter::{PositionRange, TxsFilter};
use revm::primitives::FixedBytes;

#[derive(Debug, clap::Parser)]
pub struct TxArgs {
    tx_hash: String,
    #[arg(
        long,
        short = 'B',
        help = "'before' means newer transactions (smaller indexes)"
    )]
    before: Option<u8>,
    #[arg(
        long,
        short = 'A',
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

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl TxArgs {
    pub async fn run(&self) -> Result<()> {
        let tx_hash: FixedBytes<32> = self
            .tx_hash
            .parse()
            .map_err(|_| eyre!("Invalid tx hash format: {}", self.tx_hash))?;

        check_range(self.before)?;
        check_range(self.after)?;

        let shared_deps = init_deps(&self.conn_opts).await?;
        let sqlite = shared_deps.sqlite;
        let provider = shared_deps.provider;
        let tx = provider.get_transaction_by_hash(tx_hash).await?;
        let tx = tx.ok_or_else(|| eyre!("tx {} not found", tx_hash))?;

        let block_number = tx.block_number.expect("commited tx must have block number");
        let Some(tx_index) = tx.transaction_index else {
            eyre::bail!("tx index must be present");
        };

        let revm_utils = init_revm_db(block_number - 1, &self.conn_opts).await?;
        let (mut revm_db, _anvil) = match self.conn_opts.trace {
            Some(TraceMode::Revm) => {
                let utils = revm_utils.expect("Revm must be present");
                (Some(utils.cache_db), Some(utils.anvil))
            }
            _ => (None, None),
        };

        let position_range = Some(PositionRange {
            from: 0,
            to: tx_index,
        });

        let mut mev_block = MEVBlock::new(
            block_number,
            position_range.as_ref(),
            self.reverse,
            &provider,
            self.conn_opts.trace.as_ref(),
            self.top_metadata,
        )
        .await?;

        let tx_indexes = get_matching_indexes(tx_index, self.before, self.after);

        let txs_filter = TxsFilter {
            tx_indexes: Some(tx_indexes),
            tx_from: None,
            touching: None,
            tx_position: None,
            events: vec![],
            not_events: vec![],
            match_method: None,
            tx_cost: None,
            gas_price: None,
            real_tx_cost: None,
            real_gas_price: None,
            reversed_order: self.reverse,
            top_metadata: self.top_metadata,
        };

        mev_block
            .populate_txs(
                &txs_filter,
                &sqlite,
                &ENSLookup::Sync,
                &shared_deps.symbols_lookup_worker,
                &provider,
                revm_db.as_mut(),
                &self.conn_opts,
            )
            .await?;

        println!("{}", SEPARATORER);
        print!("{}", mev_block);

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

    if let Some(count) = after {
        for i in 1..=count as u64 {
            result.insert(source_tx_index + i);
        }
    }

    if let Some(count) = before {
        for i in 1..=count as u64 {
            if source_tx_index >= i {
                result.insert(source_tx_index - i);
            }
        }
    }

    result
}

fn check_range(value: Option<u8>) -> Result<()> {
    if let Some(value) = value {
        if value > 5 {
            eyre::bail!("--before must be less than or equal 5");
        }

        if value < 1 {
            eyre::bail!("--before must be greater than 0");
        }
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
