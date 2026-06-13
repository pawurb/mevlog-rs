use eyre::Result;
use mevlog::{
    cmds,
    misc::shared_init::{ConnOpts, CryoOpts, OutputFormat, SharedOpts},
};

use crate::cmd::print_query_outcome;

#[derive(Debug, clap::Parser)]
pub struct QueryArgs {
    #[arg(short = 'b', long, help_heading = "Block number or range to collect (e.g., '22030899', 'latest', '22030800:22030900' '50:latest', '50:'", num_args(1..), required_unless_present = "skip_index", conflicts_with = "skip_index")]
    blocks: Option<String>,

    #[command(flatten)]
    shared_opts: SharedOpts,

    #[command(flatten)]
    conn_opts: ConnOpts,

    #[command(flatten)]
    cryo_opts: CryoOpts,

    #[arg(long, help = "Get N-offset latest block")]
    latest_offset: Option<u64>,

    #[arg(
        long,
        help = "Latest block number used to expand the {LATEST_BLOCK()} SQL macro, \
                avoiding the RPC call that would otherwise fetch it"
    )]
    latest_block: Option<u64>,

    #[arg(long, help = "Maximum allowed block range size")]
    max_range: Option<u64>,

    #[arg(
        long,
        help = "Maximum number of rows the --sql query may return; errors when \
                exceeded (default: unlimited)"
    )]
    max_rows: Option<usize>,

    #[arg(
        long,
        help = "Batch size for data fetching (default: 100)",
        default_value = "100"
    )]
    batch_size: usize,

    #[arg(
        long,
        help = "Skip indexing and query the local store as-is (no block range \
                resolution or fetching)"
    )]
    skip_index: bool,

    #[arg(
        long,
        help = "Read-only SQL to run against the local txs DB \
                (tables: transactions, logs, blocks). Blob columns (addresses, \
                hashes) are output as 0x-hex; addresses/hashes in predicates must \
                be given as blob literals, e.g. WHERE from_address = X'1111...1111'. \
                Macros must be wrapped in braces. {LATEST_BLOCK()} expands to the chain's \
                current latest block number (fetched via RPC), e.g. WHERE block_number > \
                {LATEST_BLOCK()} - 100. {NATIVE_TOKEN_PRICE()} expands to the native token's \
                USD price (from --native-token-price or a Chainlink oracle). \
                {RESOLVE_ENS(\"name.eth\")} expands to the resolved address as a blob literal \
                (Ethereum mainnet only), e.g. WHERE from_address = {RESOLVE_ENS(\"vitalik.eth\")}"
    )]
    sql: String,
}

impl QueryArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        let outcome = cmds::query::query(
            self.blocks.as_deref(),
            self.latest_offset,
            self.max_range,
            self.max_rows,
            self.batch_size,
            self.skip_index,
            self.latest_block,
            &self.sql,
            &self.shared_opts,
            &self.conn_opts,
            &self.cryo_opts,
        )
        .await?;
        print_query_outcome(outcome, format)
    }
}
