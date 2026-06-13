use std::path::PathBuf;

use eyre::Result;

use crate::{
    db::{
        sigs::actions::{download_file, file_exists, remove_files},
        txs,
    },
    misc::{
        config::Config,
        shared_init::{ConnOpts, resolve_conn},
    },
};

/// Re-downloads the prebuilt signatures database from the CDN, replacing any
/// existing local copy.
pub async fn update_db() -> Result<()> {
    if file_exists() {
        remove_files().await?;
    }
    download_file().await?;
    Ok(())
}

pub struct RebuildOutcome {
    pub chain_id: u64,
    /// Names of the rebuilt tables (config tables applicable to the chain).
    pub tables: Vec<String>,
}

/// Drops and rebuilds the config-defined custom tables in the resolved
/// chain's txs DB from already-indexed `logs` rows. Offline apart from chain
/// resolution; only the one chain's DB is touched — multi-chain configs need
/// one run per chain.
pub async fn rebuild_tables(conn_opts: &ConnOpts) -> Result<RebuildOutcome> {
    let resolved = resolve_conn(conn_opts).await?;
    let config = Config::load()?;
    let tables = config.custom_tables()?;

    let txs_db_url = conn_opts.txs_db_dir.as_ref().map(|dir| {
        PathBuf::from(dir)
            .join(txs::db_file_name(txs::SCHEMA_VERSION, resolved.chain_id))
            .to_string_lossy()
            .into_owned()
    });
    txs::init_db(txs_db_url.clone(), resolved.chain_id).await?;
    let pool = txs::conn(txs_db_url, resolved.chain_id, false).await?;

    let rebuilt = txs::custom_tables::rebuild(&tables, resolved.chain_id, &pool).await?;

    Ok(RebuildOutcome {
        chain_id: resolved.chain_id,
        tables: rebuilt,
    })
}
