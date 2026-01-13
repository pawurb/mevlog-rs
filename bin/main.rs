mod cmd;
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "seed-db")]
use cmd::seed_db::SeedDBArgs;
#[cfg(feature = "tui")]
use cmd::tui::TuiArgs;
use cmd::{
    chain_info::ChainInfoArgs, chains::ChainsArgs, debug_available::DebugAvailableArgs,
    search::SearchArgs, tx::TxArgs, update_db::UpdateDBArgs, watch::WatchArgs,
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
        help = "Output format ('text', 'json', 'json-pretty', 'json-stream', 'json-pretty-stream')",
        default_value = "text",
        global = true
    )]
    pub format: OutputFormat,
}

#[derive(Subcommand, Debug)]
pub enum MLSubcommand {
    #[command(about = "Monitor Ethereum transactions", alias = "w")]
    Watch(WatchArgs),
    #[command(about = "Find txs matching filter conditions", alias = "s")]
    Search(SearchArgs),
    #[command(about = "Print transaction info", alias = "t")]
    Tx(TxArgs),
    #[command(about = "Update signatures database")]
    UpdateDB(UpdateDBArgs),
    #[command(about = "List all available chains from ChainList")]
    Chains(ChainsArgs),
    #[command(about = "Show detailed chain information")]
    ChainInfo(ChainInfoArgs),
    #[command(about = "Check if RPC supports debug tracing")]
    DebugAvailable(DebugAvailableArgs),
    #[cfg(feature = "seed-db")]
    #[command(about = "[Dev] Seed signatures database from source file")]
    SeedDB(SeedDBArgs),
    #[cfg(feature = "tui")]
    #[command(about = "Run TUI")]
    Tui(TuiArgs),
}

#[cfg(feature = "hotpath-alloc")]
#[tokio::main(flavor = "current_thread")]
async fn main() {
    _ = inner_main().await;
}

#[cfg(not(feature = "hotpath-alloc"))]
#[tokio::main]
async fn main() {
    _ = inner_main().await;
}

#[hotpath::main(percentiles = [95], limit = 30)]
async fn inner_main() {
    let root_args = MLArgs::parse();

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
    match format {
        OutputFormat::Text => {
            if std::env::var("RUST_BACKTRACE").is_ok() {
                eprintln!("Error: {e:#?}");
            } else {
                eprintln!("Error: {e}");
            }
        }
        OutputFormat::Json
        | OutputFormat::JsonStream
        | OutputFormat::JsonPretty
        | OutputFormat::JsonPrettyStream => {
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

            match format {
                OutputFormat::Json | OutputFormat::JsonStream => {
                    eprintln!("{}", serde_json::to_string(&error_json).unwrap());
                }
                OutputFormat::JsonPretty | OutputFormat::JsonPrettyStream => {
                    eprintln!("{}", serde_json::to_string_pretty(&error_json).unwrap());
                }
                _ => unreachable!(),
            }
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
        ML::Watch(args) => {
            args.run(root_args.format).await?;
        }
        ML::Tx(args) => {
            args.run(root_args.format).await?;
        }
        ML::Search(args) => {
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
        ML::DebugAvailable(args) => {
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
