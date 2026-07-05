use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, CryoOpts, OutputFormat},
};

use crate::cmd::{HtmlOpts, print_query_outcome};

#[derive(Debug, clap::Parser)]
pub struct BlockTxsArgs {
    #[arg(short = 'b', long = "block", help = "Block number or 'latest'")]
    pub block: String,

    #[arg(long, help = "Get N-offset latest block")]
    pub latest_offset: Option<u64>,

    #[arg(long, help = "Native token price in USD (overrides the chain oracle)")]
    pub native_token_price: Option<f64>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,

    #[command(flatten)]
    pub cryo_opts: CryoOpts,
}

impl BlockTxsArgs {
    pub(crate) async fn run(&self, format: OutputFormat, html: &HtmlOpts) -> Result<()> {
        let outcome = cmds::block_txs::block_txs(
            &self.block,
            self.latest_offset,
            self.native_token_price,
            &self.conn_opts,
            &self.cryo_opts,
        )
        .await?;
        print_query_outcome(outcome, format, html)
    }
}
