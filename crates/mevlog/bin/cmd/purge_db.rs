use std::{path::PathBuf, time::Instant};

use eyre::{Result, bail};
use mevlog::{
    db::txs::{self, purge::purge_old_blocks},
    misc::shared_init::OutputFormat,
    models::json::purge_response::{PurgeResponse, serialize_purge_response},
};

#[derive(Debug, clap::Parser)]
pub struct PurgeDBArgs {
    #[arg(
        long,
        help = "Number of newest indexed blocks to keep; older data is deleted (0 purges everything)"
    )]
    keep: u64,

    #[arg(long, help = "Chain ID of the local transactions DB to purge")]
    chain_id: u64,

    #[arg(
        long,
        help = "Override the directory holding the per-chain transactions SQLite DB (mainly for tests); filename stays mevlog-txs-v{N}-{chain_id}.db"
    )]
    txs_db_dir: Option<String>,
}

impl PurgeDBArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        if matches!(format, OutputFormat::Csv | OutputFormat::Table) {
            bail!("'csv' and 'table' formats are only supported by the query command");
        }

        let db_url = self.txs_db_dir.as_ref().map(|dir| {
            PathBuf::from(dir)
                .join(txs::db_file_name(txs::SCHEMA_VERSION, self.chain_id))
                .to_string_lossy()
                .into_owned()
        });
        let conn = txs::conn(db_url, self.chain_id, false).await?;

        let start_time = Instant::now();
        let stats = purge_old_blocks(self.keep, &conn).await?;
        let duration_ns = start_time.elapsed().as_nanos() as u64;

        let resp = PurgeResponse::new(self.keep, self.chain_id, stats, duration_ns);

        let pretty = !matches!(format, OutputFormat::Json);
        println!("{}", serialize_purge_response(&resp, pretty)?);

        Ok(())
    }
}
