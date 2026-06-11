use alloy::primitives::TxHash;
use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, CryoOpts, OutputFormat, TraceMode},
};

use crate::cmd::print_query_outcome;

#[derive(Debug, clap::Parser)]
pub struct TxArgs {
    #[arg(help = "Transaction hash to display")]
    pub tx_hash: TxHash,

    #[arg(
        long,
        help = "EVM tracing mode ('revm' or 'rpc'); enables coinbase/full cost"
    )]
    pub evm_trace: Option<TraceMode>,

    #[arg(long, help = "Native token price in USD (overrides the chain oracle)")]
    pub native_token_price: Option<f64>,

    #[command(flatten)]
    pub conn_opts: ConnOpts,

    #[command(flatten)]
    pub cryo_opts: CryoOpts,
}

impl TxArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        let outcome = cmds::tx::tx(
            self.tx_hash,
            self.evm_trace.as_ref(),
            self.native_token_price,
            &self.conn_opts,
            &self.cryo_opts,
        )
        .await?;
        print_query_outcome(outcome, format)
    }
}
