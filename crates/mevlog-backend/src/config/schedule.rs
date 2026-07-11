use std::sync::Arc;

use eyre::Result;
use mevlog::{
    misc::shared_init::mevlog_cmd_path,
    models::json::{index_response::IndexResponse, purge_response::PurgeResponse},
};
use tokio::process::Command as AsyncCommand;
use tokio::sync::Mutex;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::misc::utils::uptime_ping;

// ~7 days of mainnet blocks (12s block time)
const PURGE_KEEP_BLOCKS: u64 = 7200;
const PURGE_CHAIN_ID: u64 = 1;
const REINDEX_CHAIN_ID: u64 = 1;

/// `job_lock` is shared with the live indexer in `bin/scheduler.rs` so that only
/// one writer touches the per-chain txs DB and cryo cache dir at a time.
pub async fn get_schedule(job_lock: Arc<Mutex<()>>) -> Result<JobScheduler> {
    let mut sched = JobScheduler::new().await?;

    sched
        .add(Job::new_async("every 10 minutes", |_uuid, _l| {
            Box::pin(async move {
                tracing::info!("Scheduler uptime ping");
                let uptime_url = std::env::var("UPTIME_URL_SCHEDULER")
                    .expect("Missing UPTIME_URL_SCHEDULER env var");
                match uptime_ping(&uptime_url).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("Failed to uptime ping: {}", &e);
                    }
                };
            })
        })?)
        .await?;

    let purge_lock = job_lock.clone();
    sched
        .add(Job::new_async("every 1 hour", move |_uuid, _l| {
            let purge_lock = purge_lock.clone();
            Box::pin(async move {
                // Wait for the lock rather than skipping: purge must run or the
                // DB grows unbounded. The Mutex is fair, so it queues behind the
                // in-flight job instead of starving.
                let _guard = purge_lock.lock().await;
                let purged = async {
                    let mut cmd = AsyncCommand::new(mevlog_cmd_path());
                    cmd.arg("purge-db")
                        .arg("--keep")
                        .arg(PURGE_KEEP_BLOCKS.to_string())
                        .arg("--chain-id")
                        .arg(PURGE_CHAIN_ID.to_string())
                        .arg("--format")
                        .arg("json");
                    cmd.env("RUST_LOG", "off");

                    let output = cmd.output().await?;
                    if !output.status.success() {
                        eyre::bail!(
                            "purge-db exited with {}: {}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let resp: PurgeResponse = serde_json::from_str(stdout.trim())?;
                    Ok::<_, eyre::Error>(resp)
                }
                .await;

                match purged {
                    Ok(resp) => {
                        tracing::info!(
                            "Purged txs DB for chain {}: {} blocks, {} txs, {} logs removed in {}",
                            PURGE_CHAIN_ID,
                            resp.purged_blocks,
                            resp.purged_transactions,
                            resp.purged_logs,
                            resp.duration
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to purge txs DB for chain {}: {}",
                            PURGE_CHAIN_ID,
                            &e
                        );
                    }
                }
            })
        })?)
        .await?;

    let reindex_lock = job_lock.clone();
    sched
        .add(Job::new_async("every 20 minutes", move |_uuid, _l| {
            let reindex_lock = reindex_lock.clone();
            Box::pin(async move {
                // Wait for the lock rather than skipping, same as purge.
                let _guard = reindex_lock.lock().await;
                let reindexed = async {
                    let rpc_url = std::env::var("ARCHIVE_ETH_RPC_URL")?;

                    let mut cmd = AsyncCommand::new(mevlog_cmd_path());
                    cmd.arg("reindex")
                        .arg("--chain-id")
                        .arg(REINDEX_CHAIN_ID.to_string())
                        .arg("--rpc-url")
                        .arg(&rpc_url)
                        .arg("--keep")
                        .arg(PURGE_KEEP_BLOCKS.to_string())
                        .arg("--format")
                        .arg("json");
                    cmd.env("RUST_LOG", "off");

                    let output = cmd.output().await?;
                    if !output.status.success() {
                        eyre::bail!(
                            "reindex exited with {}: {}",
                            output.status,
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }

                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let resp: IndexResponse = serde_json::from_str(stdout.trim())?;
                    Ok::<_, eyre::Error>(resp)
                }
                .await;

                match reindexed {
                    Ok(resp) => {
                        tracing::info!(
                            "Reindexed txs DB for chain {}: {} blocks refetched, {} cached in {}",
                            REINDEX_CHAIN_ID,
                            resp.new_blocks,
                            resp.cached_blocks,
                            resp.duration
                        );
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to reindex txs DB for chain {}: {}",
                            REINDEX_CHAIN_ID,
                            &e
                        );
                    }
                }
            })
        })?)
        .await?;

    sched.shutdown_on_ctrl_c();

    sched.set_shutdown_handler(Box::new(|| {
        Box::pin(async move {
            tracing::info!("Shut down done");
        })
    }));

    Ok(sched)
}
