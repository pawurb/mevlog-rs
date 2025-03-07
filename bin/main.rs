mod cmd;
use clap::{Parser, Subcommand};
#[cfg(feature = "dev")]
use cmd::seed_db::SeedDBArgs;

use cmd::{search::SearchArgs, tx::TxArgs, update_db::UpdateDBArgs, watch::WatchArgs};
use eyre::Result;
use mevlog::misc::utils::init_logs;

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
}
#[derive(Subcommand, Debug)]
pub enum MLSubcommand {
    #[command(about = "Monitor Ethereum transactions", alias = "w")]
    #[command(about = "Find txs matching filter conditions", alias = "s")]
    Search(SearchArgs),
    Watch(WatchArgs),
    #[command(about = "Print transaction info", alias = "t")]
    Tx(TxArgs),
    #[command(about = "Update signatures database")]
    UpdateDB(UpdateDBArgs),
    #[cfg(feature = "dev")]
    #[command(about = "[Dev] Seed signatures database from source file")]
    SeedDB(SeedDBArgs),
}

#[tokio::main]
async fn main() {
    init_logs();
    match execute().await {
        Ok(_) => {}
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

type ML = MLSubcommand;

async fn execute() -> Result<()> {
    let args = MLArgs::parse();

    match args.cmd {
        ML::Watch(args) => {
            args.run().await?;
        }
        ML::Tx(args) => {
            args.run().await?;
        }
        ML::Search(args) => {
            args.run().await?;
        }
        ML::UpdateDB(args) => {
            args.run().await?;
        }
        #[cfg(feature = "dev")]
        ML::SeedDB(args) => {
            args.run().await?;
        }
    }

    Ok(())
}
