mod cmd;
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "mcp")]
use cmd::mcp::McpArgs;
#[cfg(feature = "seed-db")]
use cmd::seed_db::SeedDBArgs;
#[cfg(feature = "tui")]
use cmd::tui::TuiArgs;
use cmd::{
    affected_addresses::AffectedAddressesArgs, chain_info::ChainInfoArgs, chains::ChainsArgs,
    coinbase_transfer::CoinbaseTransferArgs, debug_available::DebugAvailableArgs,
    ens_lookup::EnsLookupArgs, ens_resolve::EnsResolveArgs, evm_traces::EvmTracesArgs,
    query::QueryArgs, state_diff::StateDiffArgs, tx::TxArgs, tx_logs::TxLogsArgs,
    update_db::UpdateDBArgs,
};
use eyre::Result;
use mevlog::misc::shared_init::OutputFormat;

#[derive(Clone, Debug, ValueEnum)]
pub enum ColorMode {
    Always,
    Auto,
    Never,
}

#[derive(Parser, Debug)]
#[command(
    version,
    about,
    long_about = "mevlog: EVM activity log monitoring CLI

https://github.com/pawurb/mevlog-rs"
)]
pub struct MLArgs {
    #[command(subcommand)]
    pub cmd: MLSubcommand,

    #[arg(long, value_enum, default_value = "auto", global = true)]
    pub color: ColorMode,

    #[arg(
        long,
        help = "Output format ('json', 'json-pretty', 'csv', 'table'); 'csv' and 'table' are query-only",
        default_value = "json-pretty",
        global = true
    )]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug)]
pub enum MLSubcommand {
    #[command(about = "Query txs within a block range", alias = "q")]
    Query(Box<QueryArgs>),
    #[command(about = "Show a single transaction")]
    Tx(TxArgs),
    #[command(name = "tx-logs", about = "Show a transaction's logs")]
    TxLogs(TxLogsArgs),
    #[command(about = "Update signatures database")]
    UpdateDB(UpdateDBArgs),
    #[command(about = "List all available chains from ChainList")]
    Chains(ChainsArgs),
    #[command(about = "Show detailed chain information")]
    ChainInfo(ChainInfoArgs),
    #[command(
        name = "evm-coinbase-transfer",
        about = "Compute a tx's direct ETH payment to its block's coinbase"
    )]
    CoinbaseTransfer(CoinbaseTransferArgs),
    #[command(
        name = "evm-affected-addresses",
        about = "List addresses affected by a tx"
    )]
    AffectedAddresses(AffectedAddressesArgs),
    #[command(
        name = "evm-state-diff",
        about = "Show the storage state diff produced by a tx"
    )]
    StateDiff(StateDiffArgs),
    #[command(name = "evm-traces", about = "Extract a tx's decoded call traces")]
    EvmTraces(EvmTracesArgs),
    #[command(about = "Check if RPC supports debug tracing")]
    DebugAvailable(DebugAvailableArgs),
    #[command(about = "Resolve an ENS name to an address")]
    EnsResolve(EnsResolveArgs),
    #[command(about = "Reverse-resolve an address to an ENS name")]
    EnsLookup(EnsLookupArgs),
    #[cfg(feature = "mcp")]
    #[command(about = "Start MCP server")]
    Mcp(McpArgs),
    #[cfg(feature = "seed-db")]
    #[command(about = "[Dev] Seed signatures database from the Sourcify export")]
    SeedDB(SeedDBArgs),
    #[cfg(feature = "tui")]
    #[command(about = "Run TUI")]
    Tui(TuiArgs),
}

#[tokio::main]
#[hotpath::main(percentiles = [95], limit = 15)]
async fn main() {
    let root_args = MLArgs::parse();
    hotpath::tokio_runtime!();

    #[cfg(feature = "tui")]
    mevlog::misc::utils::init_file_logs();
    #[cfg(not(feature = "tui"))]
    mevlog::misc::utils::init_std_logs();

    let format = root_args.format.clone();

    match execute(root_args).await {
        Ok(_) => {}
        Err(e) => {
            print_error(&e, &format);
            std::process::exit(1);
        }
    }
}

fn print_error(e: &eyre::Error, format: &OutputFormat) {
    let error_json = if std::env::var("RUST_BACKTRACE").is_ok() {
        serde_json::json!({
            "error": e.to_string(),
            "backtrace": format!("{e:#?}")
        })
    } else {
        serde_json::json!({
            "error": e.to_string()
        })
    };

    // Errors are not tabular; csv/table fall back to compact JSON on stderr.
    match format {
        OutputFormat::JsonPretty => {
            eprintln!("{}", serde_json::to_string_pretty(&error_json).unwrap());
        }
        OutputFormat::Json | OutputFormat::Csv | OutputFormat::Table => {
            eprintln!("{}", serde_json::to_string(&error_json).unwrap());
        }
    }
}

type ML = MLSubcommand;

async fn execute(root_args: MLArgs) -> Result<()> {
    match root_args.color {
        ColorMode::Always => colored::control::set_override(true),
        ColorMode::Never => colored::control::set_override(false),
        ColorMode::Auto => {}
    }

    match root_args.cmd {
        ML::Query(args) => {
            args.run(root_args.format).await?;
        }
        ML::Tx(args) => {
            args.run(root_args.format).await?;
        }
        ML::TxLogs(args) => {
            args.run(root_args.format).await?;
        }
        ML::UpdateDB(args) => {
            args.run().await?;
        }
        ML::Chains(args) => {
            args.run(root_args.format).await?;
        }
        ML::ChainInfo(args) => {
            args.run(root_args.format).await?;
        }
        ML::CoinbaseTransfer(args) => {
            args.run(root_args.format).await?;
        }
        ML::AffectedAddresses(args) => {
            args.run(root_args.format).await?;
        }
        ML::StateDiff(args) => {
            args.run(root_args.format).await?;
        }
        ML::EvmTraces(args) => {
            args.run(root_args.format).await?;
        }
        ML::DebugAvailable(args) => {
            args.run().await?;
        }
        ML::EnsResolve(args) => {
            args.run(root_args.format).await?;
        }
        ML::EnsLookup(args) => {
            args.run(root_args.format).await?;
        }
        #[cfg(feature = "mcp")]
        ML::Mcp(args) => {
            args.run().await?;
        }
        #[cfg(feature = "seed-db")]
        ML::SeedDB(args) => {
            args.run().await?;
        }
        #[cfg(feature = "tui")]
        ML::Tui(args) => {
            args.run().await?;
        }
    }

    Ok(())
}

#[cfg(all(test, feature = "mcp"))]
mod tests {
    use super::{MLArgs, MLSubcommand};
    use clap::Parser;

    #[test]
    fn query_subcommand_accepts_conn_flags_after_subcommand_name() {
        let parsed = MLArgs::try_parse_from([
            "mevlog",
            "--format",
            "json",
            "query",
            "-b",
            "10:latest",
            "--rpc-url",
            "http://localhost:8545",
            "--chain-id",
            "1",
        ])
        .expect("query args should parse");

        match parsed.cmd {
            MLSubcommand::Query(_) => {}
            other => panic!("expected query command, got {other:?}"),
        }
    }
}
