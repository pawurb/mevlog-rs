use eyre::Result;
use std::time::{Duration, Instant};

pub fn deployed_at() -> String {
    std::env::var("DEPLOYED_AT").unwrap_or_else(|_| "unknown".to_string())
}

pub fn measure_start(label: &str) -> (String, Instant) {
    (label.to_string(), Instant::now())
}

pub fn measure_end(start: (String, Instant)) -> Duration {
    let elapsed = start.1.elapsed();
    tracing::info!("Elapsed: {:.2?} for '{}'", elapsed, start.0);
    elapsed
}

pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let millis = duration.subsec_millis();
    format!("{secs}.{millis:02}s")
}

#[hotpath::measure]
pub async fn uptime_ping(uptime_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    match client.get(uptime_url).send().await {
        Ok(_) => {}
        Err(e) => {
            tracing::error!("Failed to uptime ping: {}", &e);
        }
    };
    Ok(())
}
