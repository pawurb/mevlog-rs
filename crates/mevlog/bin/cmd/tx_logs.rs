use alloy::primitives::TxHash;
use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat},
};

use crate::cmd::print_query_outcome;

#[derive(Debug, clap::Parser)]
pub struct TxLogsArgs {
    #[arg(help = "Transaction hash whose logs to display")]
    pub tx_hash: TxHash,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl TxLogsArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let outcome = cmds::tx_logs::tx_logs(self.tx_hash, &self.conn_opts).await?;
        print_query_outcome(outcome, format)
    }
}
