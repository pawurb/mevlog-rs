use eyre::{Result, bail};
use mevlog::{
    db::txs::{self, info::db_info},
    misc::shared_init::OutputFormat,
    models::json::db_info_response::{DbInfoResponse, serialize_db_info_response},
};

#[derive(Debug, clap::Parser)]
pub struct DbInfoArgs {
    #[arg(long, help = "Chain ID of the local transactions DB to inspect")]
    chain_id: u64,

    #[arg(
        long,
        help = "Override the directory holding the per-chain transactions SQLite DB (mainly for tests); filename stays mevlog-txs-v{N}-{chain_id}.db"
    )]
    txs_db_dir: Option<String>,
}

impl DbInfoArgs {
    pub(crate) async fn run(&self, format: OutputFormat) -> Result<()> {
        if matches!(
            format,
            OutputFormat::Csv | OutputFormat::Table | OutputFormat::Html
        ) {
            bail!("'csv', 'table' and 'html' formats are only supported by the query command");
        }

        let db_path = txs::resolve_db_path(self.txs_db_dir.as_deref(), self.chain_id);
        if !db_path.exists() {
            bail!("Txs DB not found at {}", db_path.display());
        }
        let db_size_bytes = std::fs::metadata(&db_path)?.len();
        let wal_size_bytes = std::fs::metadata(db_path.with_extension("db-wal"))
            .map(|m| m.len())
            .unwrap_or(0);

        let conn = txs::conn(
            Some(db_path.to_string_lossy().into_owned()),
            self.chain_id,
            true,
        )
        .await?;

        let stats = db_info(&conn).await?;

        let resp = DbInfoResponse::new(
            self.chain_id,
            db_path.to_string_lossy().into_owned(),
            txs::SCHEMA_VERSION,
            db_size_bytes,
            wal_size_bytes,
            stats,
        );

        let pretty = !matches!(format, OutputFormat::Json);
        println!("{}", serialize_db_info_response(&resp, pretty)?);

        Ok(())
    }
}
