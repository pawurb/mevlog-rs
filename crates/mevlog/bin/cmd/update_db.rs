use eyre::Result;
use mevlog::cmds;

#[derive(Debug, clap::Parser)]
pub struct UpdateDBArgs {}

impl UpdateDBArgs {
    pub async fn run(&self) -> Result<()> {
        cmds::update_db::update_db().await
    }
}
