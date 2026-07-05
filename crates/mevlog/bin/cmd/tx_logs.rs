use alloy::primitives::TxHash;
use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, CryoOpts, OutputFormat},
};

use crate::cmd::{HtmlOpts, print_query_outcome};

#[derive(Debug, clap::Parser)]
pub struct TxLogsArgs {
    #[arg(help = "Transaction hash whose logs to display")]
    pub tx_hash: TxHash,

    #[command(flatten)]
    pub conn_opts: ConnOpts,

    #[command(flatten)]
    pub cryo_opts: CryoOpts,
}

impl TxLogsArgs {
    pub(crate) async fn run(
        &self,
        format: OutputFormat,
        html: &HtmlOpts,
        ipfs: bool,
    ) -> Result<()> {
        let outcome =
            cmds::tx_logs::tx_logs(self.tx_hash, &self.conn_opts, &self.cryo_opts).await?;
        print_query_outcome(outcome, format, html, ipfs).await
    }
}
