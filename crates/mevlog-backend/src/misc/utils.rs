use eyre::Result;

pub(crate) fn deployed_at() -> String {
    std::env::var("DEPLOYED_AT").unwrap_or_else(|_| "unknown".to_string())
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
