use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use clap::Parser;
use eyre::Result;
use mevlog::{
    misc::database::{init_sqlite_db, sqlite_conn, sqlite_truncate_wal},
    models::{db_event::NewEvent, db_method::NewMethod},
};
use tracing::info;

#[derive(Debug, Parser)]
pub struct SeedDBArgs {}

impl SeedDBArgs {
    #[allow(dead_code)]
    pub async fn run(&self) -> Result<()> {
        println!("Seeding db");
        init_sqlite_db(None).await?;
        let conn = sqlite_conn(None).await?;

        tracing::info!("Seeding signature database");

        let file_path = std::env::var("SEED_FILE_URL").expect("SEED_FILE_URL must be set");

        let file1 = File::open(file_path.clone())?;
        let file2 = File::open(file_path)?;
        let count_reader = BufReader::new(file1);
        let reader = BufReader::new(file2);

        let total_lines = count_reader.lines().count();

        for (i, line) in reader.lines().enumerate() {
            let line = line?;
            let Some((signature_hash, signature)) = line.split_once(',') else {
                continue;
            };

            if i % 10000 == 0 {
                tracing::info!("Processing line {}/{}", i, total_lines);
            }

            if signature_hash.len() == 10 {
                let new_method = NewMethod {
                    signature: signature.to_string(),
                    signature_hash: signature_hash.to_string(),
                };

                let _ = new_method.save(&conn).await;
            }

            if signature_hash.len() == 66 {
                let new_event = NewEvent {
                    signature: signature.to_string(),
                    signature_hash: signature_hash.to_string(),
                };

                let _ = new_event.save(&conn).await;
            }
        }

        info!("Truncating WAL");

        sqlite_truncate_wal(&conn).await?;

        info!("Finished seeding signatures database");

        Ok(())
    }
}
