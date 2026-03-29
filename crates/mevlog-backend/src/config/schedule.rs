use eyre::Result;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::misc::{prices::update_prices_cache, utils::uptime_ping};

pub async fn get_schedule() -> Result<JobScheduler> {
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

    sched
        .add(Job::new_async("every 5 minutes", |_uuid, _l| {
            Box::pin(async move {
                match update_prices_cache().await {
                    Ok(_) => {
                        tracing::info!("Prices cache updated");
                    }
                    Err(e) => {
                        tracing::error!("Failed to update prices cache: {}", &e);
                    }
                }
            })
        })?)
        .await?;

    // sched
    //     .add(Job::new_async("every 2 minutes", |_uuid, _l| {
    //         Box::pin(async move {
    //             tracing::info!("Block number check");
    //             let local_rpc_url =
    //                 std::env::var("ETH_RPC_URL_VAL").expect("Missing ETH_RPC_URL_VAL env var");
    //             let local_provider = ProviderBuilder::new()
    //                 .on_http(local_rpc_url.parse().expect("Invalid local RPC URL"));

    //             let remote_rpc_url = std::env::var("ETH_RPC_URL_REMOTE")
    //                 .expect("Missing ETH_RPC_URL_REMOTE env var");
    //             let remote_provider = ProviderBuilder::new()
    //                 .on_http(remote_rpc_url.parse().expect("Invalid remote RPC URL"));

    //             let local_block_number = match local_provider.get_block_number().await {
    //                 Ok(n) => n,
    //                 Err(e) => {
    //                     tracing::error!("Failed to get local block number: {}", e);
    //                     return;
    //                 }
    //             };
    //             let remote_block_number = match remote_provider.get_block_number().await {
    //                 Ok(n) => n,
    //                 Err(e) => {
    //                     tracing::error!("Failed to get remote block number: {}", e);
    //                     return;
    //                 }
    //             };

    //             if local_block_number + 1 >= remote_block_number {
    //                 let uptime_url = std::env::var("UPTIME_URL_BLOCK_NUMBER")
    //                     .expect("Missing UPTIME_URL_BLOCK_NUMBER env var");
    //                 tracing::info!("Blocks number matching ping");

    //                 match uptime_ping(&uptime_url).await {
    //                     Ok(_) => {}
    //                     Err(e) => {
    //                         error!("Failed to uptime ping: {}", &e);
    //                     }
    //                 }
    //             }
    //         })
    //     })?)
    //     .await?;

    // sched
    //     .add(Job::new_async("every 30 minutes", |_uuid, _l| {
    //         Box::pin(async move {
    //             let rpc_url =
    //                 std::env::var("ETH_RPC_URL_VAL").expect("Missing ETH_RPC_URL_VAL env var");
    //             let provider = ProviderBuilder::new().on_http(rpc_url.parse().unwrap());

    //             let target_dir = "/root/.foundry/cache/rpc/mainnet";

    //             let block_number = match provider.get_block_number().await {
    //                 Ok(n) => n,
    //                 Err(e) => {
    //                     tracing::error!("Failed to get block number: {}", e);
    //                     return;
    //                 }
    //             };

    //             let threshold = block_number - BLOCKS_CACHE_THRESHOLD;

    //             tracing::info!(
    //                 "Running cleanup in '{}', removing files with name < {}",
    //                 target_dir,
    //                 threshold
    //             );

    //             let Ok(entries) = fs::read_dir(target_dir) else {
    //                 tracing::error!("Failed to read directory '{}'", target_dir);
    //                 return;
    //             };

    //             for entry in entries.flatten() {
    //                 let file_name = entry.file_name();
    //                 let file_name = file_name.to_str().unwrap();

    //                 let Ok(file_block_num) = file_name.parse::<u64>() else {
    //                     continue;
    //                 };

    //                 if file_block_num > threshold {
    //                     continue;
    //                 }

    //                 let path = entry.path();
    //                 match fs::remove_file(&path) {
    //                     Ok(_) => tracing::info!("Deleted file: {}", file_name),

    //                     Err(e) => {
    //                         tracing::error!("Failed to delete {}: {}", file_name, e)
    //                     }
    //                 }
    //             }
    //         })
    //     })?)
    //     .await?;

    sched.shutdown_on_ctrl_c();

    sched.set_shutdown_handler(Box::new(|| {
        Box::pin(async move {
            tracing::info!("Shut down done");
        })
    }));

    Ok(sched)
}
