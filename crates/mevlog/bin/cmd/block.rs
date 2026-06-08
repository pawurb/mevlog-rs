use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, OutputFormat},
};

use crate::cmd::print_query_outcome;

#[derive(Debug, clap::Parser)]
pub struct BlockArgs {
    #[arg(help = "Block number or 'latest'")]
    pub block: String,

    #[arg(long, help = "Get N-offset latest block")]
    pub latest_offset: Option<u64>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl BlockArgs {
    pub async fn run(&self, format: OutputFormat) -> Result<()> {
        let outcome = cmds::block::block(&self.block, self.latest_offset, &self.conn_opts).await?;
        print_query_outcome(outcome, format)
    }
}
