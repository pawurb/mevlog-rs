use eyre::Result;
use mevlog::cmds;

#[derive(Debug, clap::Parser)]
pub struct UpdateSigsDBArgs {}

impl UpdateSigsDBArgs {
    pub(crate) async fn run(&self) -> Result<()> {
        cmds::update_db::update_db().await
    }
}
