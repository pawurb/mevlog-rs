use std::sync::Arc;

use futures::FutureExt;
use mevlog::misc::shared_init::mevlog_cmd_path;
use tokio::process::Command;
use tokio::sync::Mutex;

use alloy::providers::{Provider, ProviderBuilder};
use eyre::Result;
use mevlog_backend::config::{middleware, schedule::get_schedule};
use mevlog_backend::misc::utils::uptime_ping;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() -> Result<()> {
    match run().await {
        Ok(_) => Ok(()),
        Err(e) => {
            tracing::error!("{:?}", e);
            Err(e)
        }
    }
}

async fn run() -> Result<()> {
    middleware::init_logs("scheduler.log");

    // Shared across the live indexer and the scheduled reindex/purge jobs so that
    // only one writer touches the per-chain txs DB and cryo cache dir at a time.
    let job_lock = Arc::new(Mutex::new(()));

    let cache_lock = job_lock.clone();
    tokio::spawn(async move {
        let result = std::panic::AssertUnwindSafe(populate_mainnet_cache(cache_lock))
            .catch_unwind()
            .await;

        match result {
            Ok(Ok(_)) => panic!("Cache task finished cleanly (which it never should)"),
            Ok(Err(e)) => error!("Cache task errored: {:?}", e),
            Err(e) => error!("Cache task panicked: {:?}", e),
        }
    });

    let sched = get_schedule(job_lock).await?;
    sched.start().await?;

    tokio::signal::ctrl_c().await?;

    info!("Scheduler ending");

    Ok(())
}

async fn populate_mainnet_cache(job_lock: Arc<Mutex<()>>) -> Result<()> {
    let rpc_url = std::env::var("REMOTE_ETH_RPC_URL").expect("Missing REMOTE_ETH_RPC_URL env var");
    let uptime_url = std::env::var("UPTIME_URL_MAINNET_CACHE")
        .expect("Missing UPTIME_URL_MAINNET_CACHE env var");
    let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
    info!("Scheduler connected to HTTP provider");

    let mut last_indexed = provider.get_block_number().await?;
    loop {
        let latest = match provider.get_block_number().await {
            Ok(block_number) => block_number,
            Err(e) => {
                error!("Failed to get block number: {}", &e);
                tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                continue;
            }
        };

        if latest <= last_indexed {
            debug!("No new blocks, sleeping: {}", last_indexed);
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            continue;
        }

        let range = format!("{}:{}", last_indexed + 1, latest);
        // Per-block updates yield to the sync jobs: if reindex/purge holds the
        // lock, skip this round and retry. `last_indexed` stays put, so the next
        // round re-covers this range once the lock is free.
        let status = {
            let _guard = match job_lock.try_lock() {
                Ok(guard) => guard,
                Err(_) => {
                    debug!("Sync job running, deferring block index: {}", range);
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                    continue;
                }
            };
            match Command::new(mevlog_cmd_path())
                .arg("index")
                .arg("-b")
                .arg(&range)
                .arg("--rpc-url")
                .arg(&rpc_url)
                .status()
                .await
            {
                Ok(status) => status,
                Err(e) => {
                    error!("Failed to run mevlog index: {}", &e);
                    continue;
                }
            }
        };

        if !status.success() {
            error!("mevlog index {} exited: {}", range, status);
            continue;
        }

        last_indexed = latest;

        info!("Mainnet cache uptime ping");
        if let Err(e) = uptime_ping(&uptime_url).await {
            error!("Failed to uptime ping: {}", &e);
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
