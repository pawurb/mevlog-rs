use eyre::Result;
use mevlog::{cmds, misc::shared_init::ConnOpts};

#[derive(Debug, clap::Parser)]
pub struct UpdateDBArgs {
    #[arg(
        long,
        help = "Drop and rebuild config-defined custom tables from indexed logs (requires --chain-id or --rpc-url; one run per chain)"
    )]
    rebuild_tables: bool,

    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl UpdateDBArgs {
    pub(crate) async fn run(&self) -> Result<()> {
        if !self.rebuild_tables {
            return cmds::update_db::update_db().await;
        }

        let outcome = cmds::update_db::rebuild_tables(&self.conn_opts).await?;
        if outcome.tables.is_empty() {
            println!("No custom tables configured for chain {}", outcome.chain_id);
        } else {
            println!(
                "Rebuilt custom tables for chain {}: {}",
                outcome.chain_id,
                outcome.tables.join(", ")
            );
        }
        Ok(())
    }
}
