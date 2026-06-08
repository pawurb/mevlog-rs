use eyre::Result;
use serde::Deserialize;

pub const DEFAULT_BLOCKS: &str = "latest";

pub(crate) fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    Ok(opt.filter(|s| !s.trim().is_empty()))
}

pub(crate) fn error_message(e: &str) -> String {
    format!("<div class='error'>{e}</div>")
}

pub(crate) fn get_default_blocks(blocks: Option<String>) -> String {
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

pub(crate) fn decorate_error_message(e: &str) -> String {
    if e.contains("No matching") {
        DATA_FETCH_ERROR.to_string()
    } else {
        e.to_string()
    }
}
