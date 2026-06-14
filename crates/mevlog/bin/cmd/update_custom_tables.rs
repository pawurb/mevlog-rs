use eyre::Result;
use mevlog::{cmds, misc::shared_init::ConnOpts};

#[derive(Debug, clap::Parser)]
pub struct UpdateCustomTablesArgs {
    #[command(flatten)]
    conn_opts: ConnOpts,
}

impl UpdateCustomTablesArgs {
    pub(crate) async fn run(&self) -> Result<()> {
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
