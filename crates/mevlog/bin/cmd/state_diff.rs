use alloy::primitives::{B256, TxHash};
use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat, TraceMode},
    models::json::state_diff_json::StateDiffJson,
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
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        let state_diff =
            cmds::state_diff::state_diff(self.tx_hash, self.evm_trace.as_ref(), &self.conn_opts)
                .await?;

        match format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string(&StateDiffJson::from(&state_diff))?
                )
            }
            OutputFormat::JsonPretty => println!(
                "{}",
                serde_json::to_string_pretty(&StateDiffJson::from(&state_diff))?
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
            OutputFormat::Html => {
                eyre::bail!("'html' format is only supported by the query command")
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
