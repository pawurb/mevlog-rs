use futures::FutureExt;
use tokio::process::Command;

use alloy::providers::{Provider, ProviderBuilder};
use eyre::Result;
use mevlog_backend::config::{middleware, schedule::get_schedule};
use mevlog_backend::misc::utils::{measure_end, measure_start, uptime_ping};
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
    tokio::spawn(async move {
        let result = std::panic::AssertUnwindSafe(populate_mainnet_cache())
            .catch_unwind()
            .await;

        match result {
            Ok(Ok(_)) => panic!("Cache task finished cleanly (which it never should)"),
            Ok(Err(e)) => error!("Cache task errored: {:?}", e),
            Err(e) => error!("Cache task panicked: {:?}", e),
        }
    });

    let sched = get_schedule().await?;
    sched.start().await?;

    tokio::signal::ctrl_c().await?;

    info!("Scheduler ending");

    Ok(())
}

async fn populate_mainnet_cache() -> Result<()> {
    let rpc_url = std::env::var("REMOTE_ETH_RPC_URL").expect("Missing REMOTE_ETH_RPC_URL env var");
    let provider = ProviderBuilder::new().connect_http(rpc_url.parse()?);
    tracing::info!("Scheduler connected to HTTP provider");

    let mut current_block_number = provider.get_block_number().await?;
    loop {
        let new_block_number = match provider.get_block_number().await {
            Ok(block_number) => block_number,
            Err(e) => {
                error!("Failed to get block number: {}", &e);
                continue;
            }
        };

        if new_block_number == current_block_number {
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            debug!("No new blocks, sleeping: {}", current_block_number);
            continue;
        }

        current_block_number = new_block_number;

        let start = measure_start("mevlog latest");
        let _resp = match Command::new("mevlog")
            .arg("search")
            .arg("-b")
            .arg("latest")
            .arg("--rpc-url")
            .arg(rpc_url.clone())
            .output()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to run mevlog search latest: {}", &e);
                continue;
            }
        };
        measure_end(start);

        if new_block_number % 10 == 0 {
            let uptime_url = std::env::var("UPTIME_URL_MAINNET_CACHE")
                .expect("Missing UPTIME_URL_MAINNET_CACHE env var");
            info!("Mainnet cache uptime ping");

            match uptime_ping(&uptime_url).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to uptime ping: {}", &e);
                }
            }
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
