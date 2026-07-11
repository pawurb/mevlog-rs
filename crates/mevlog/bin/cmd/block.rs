use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, CryoOpts},
};

use crate::cmd::{RenderOpts, print_query_outcome};

#[derive(Debug, clap::Parser)]
pub struct BlockArgs {
    #[arg(short = 'b', long = "block", help = "Block number or 'latest'")]
    pub block: String,

    #[arg(long, help = "Get N-offset latest block")]
    pub latest_offset: Option<u64>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,

    #[command(flatten)]
    pub cryo_opts: CryoOpts,
}

impl BlockArgs {
    pub(crate) async fn run(&self, render: &RenderOpts) -> Result<()> {
        let outcome = cmds::block::block(
            &self.block,
            self.latest_offset,
            &self.conn_opts,
            &self.cryo_opts,
        )
        .await?;
        print_query_outcome(outcome, render).await
    }
}
