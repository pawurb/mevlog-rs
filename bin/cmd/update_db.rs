use eyre::Result;
use mevlog::misc::db_actions::{db_file_exists, download_db_file, remove_db_files};

#[derive(Debug, clap::Parser)]
pub struct UpdateDBArgs {}

impl UpdateDBArgs {
    pub async fn run(&self) -> Result<()> {
        if db_file_exists() {
            remove_db_files().await?;
        }
        download_db_file().await?;
        Ok(())
    }
}
