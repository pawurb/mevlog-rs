use alloy::primitives::{B256, TxHash};
use eyre::{Result, bail};
use mevlog::{
    db::sigs::models::chain::Chain,
    misc::{
        shared_init::{ConnOpts, OutputFormat, TraceMode, resolve_conn},
        tx_tracing::state_diff_for_tx,
    },
    models::{evm_chain::EVMChain, json::mev_state_diff_json::MEVStateDiffJson},
};

#[derive(Debug, clap::Parser)]
pub struct StateDiffArgs {
    #[arg(help = "Transaction hash to compute the storage state diff for")]
    pub tx_hash: TxHash,

    #[arg(long, help = "EVM tracing mode ('revm' or 'rpc')")]
    pub evm_trace: Option<TraceMode>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl StateDiffArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let Some(mode) = &self.evm_trace else {
            bail!("--evm-trace [rpc|revm] must be specified")
        };

        let conn = resolve_conn(&self.conn_opts).await?;
        let chain = EVMChain::new(Chain::unknown(conn.chain_id as i64), conn.rpc_url.clone())?;

        let state_diff =
            state_diff_for_tx(self.tx_hash, mode, &conn.provider, &chain, &conn.rpc_url).await?;

        match format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string(&MEVStateDiffJson::from(&state_diff))?
                )
            }
            OutputFormat::JsonPretty => println!(
                "{}",
                serde_json::to_string_pretty(&MEVStateDiffJson::from(&state_diff))?
            ),
            OutputFormat::Table => {
                if state_diff.is_empty() {
                    println!("No storage changes");
                } else {
                    for (address, changes) in &state_diff.contracts {
                        println!("{address}");
                        for change in changes {
                            println!("  {}", change.slot);
                            println!("    Before: {}", fmt_opt(change.value_before));
                            println!("    After:  {}", fmt_opt(change.value_after));
                        }
                    }
                }
            }
            OutputFormat::Csv => {
                println!("address,slot,value_before,value_after");
                for (address, changes) in &state_diff.contracts {
                    for change in changes {
                        println!(
                            "{address},{},{},{}",
                            change.slot,
                            fmt_opt(change.value_before),
                            fmt_opt(change.value_after),
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

fn fmt_opt(value: Option<B256>) -> String {
    value
        .map(|v| format!("{v}"))
        .unwrap_or_else(|| "null".to_string())
}
