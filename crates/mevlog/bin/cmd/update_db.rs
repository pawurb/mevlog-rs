use eyre::Result;
use mevlog::db::sigs::actions::{download_file, file_exists, remove_files};

#[derive(Debug, clap::Parser)]
pub struct UpdateDBArgs {}

impl UpdateDBArgs {
    pub async fn run(&self) -> Result<()> {
        if file_exists() {
            remove_files().await?;
        }
        download_file().await?;
        Ok(())
    }
}
