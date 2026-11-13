pub(crate) fn deployed_at() -> String {
    std::env::var("DEPLOYED_AT").unwrap_or_else(|_| "unknown".to_string())
}
