use eyre::Result;
use serde::Deserialize;

pub const DEFAULT_BLOCKS: &str = "latest";

pub fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s.as_deref() {
        Some("true") | Some("1") | Some("on") | Some("yes") => Ok(true),
        Some("false") | Some("0") | Some("off") | Some("no") => Ok(false),
        Some(other) => Err(D::Error::custom(format!("Invalid boolean value: {other}"))),
        None => Ok(false),
    }
}

pub fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.trim().is_empty()))
}

pub fn loading_spinner() -> String {
    "<div class='spinner-container'><div class='spinner'></div><div>Loading...</div></div>"
        .to_string()
}

pub fn error_message(e: &str) -> String {
    format!("<div class='error'>{e}</div>")
}

pub fn get_default_blocks(blocks: Option<String>) -> String {
    match blocks {
        Some(blocks) => {
            if blocks.is_empty() {
                DEFAULT_BLOCKS.to_string()
            } else {
                blocks
            }
        }
        None => DEFAULT_BLOCKS.to_string(),
    }
}

pub static DATA_FETCH_ERROR: &str = "Data fetch failed. This is expected because we're using public RPCs. Please try again or select a different chain.";

pub fn decorate_error_message(e: &str) -> String {
    if e.contains("No matching") {
        DATA_FETCH_ERROR.to_string()
    } else {
        e.to_string()
    }
}
