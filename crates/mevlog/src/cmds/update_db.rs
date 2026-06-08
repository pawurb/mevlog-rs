use eyre::Result;

use crate::db::sigs::actions::{download_file, file_exists, remove_files};

/// Re-downloads the prebuilt signatures database from the CDN, replacing any
/// existing local copy.
pub async fn update_db() -> Result<()> {
    if file_exists() {
        remove_files().await?;
    }
    download_file().await?;
    Ok(())
}
