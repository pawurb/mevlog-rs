//! IPFS upload backends for the `--ipfs` flag.
//!
//! Two backends are supported (selected by [`IpfsConfig::backend`]):
//! - **Pinata** - managed pinning service, a single multipart POST to the V3
//!   Files API (`uploads.pinata.cloud/v3/files`, `network=public`) authenticated
//!   with a JWT that has the `Files: Write` scope. Gives a persistent shareable
//!   link with no local infrastructure.
//! - **Kubo** - the reference IPFS node's native RPC (`/api/v0/add`). No
//!   account and no vendor, but requires a running local daemon.
//!
//! There is no vendor-neutral "upload bytes" standard in IPFS: the one
//! standardized HTTP API (the Pinning Service API) pins an existing CID and
//! cannot accept raw bytes, so each backend uses its own add endpoint.

use eyre::{Result, bail, eyre};

use crate::misc::config::{IpfsBackendKind, IpfsConfig};

// Content is uploaded with network=public, so any public gateway resolves it.
// Pinata's shared gateway.pinata.cloud only serves a dedicated-gateway plan's
// own CIDs (403s otherwise), so default to a public gateway that always works;
// users with a dedicated Pinata gateway set `ipfs.gateway` to override.
const PINATA_DEFAULT_GATEWAY: &str = "https://ipfs.io";
const KUBO_DEFAULT_GATEWAY: &str = "https://ipfs.io";

/// Outcome of a successful upload: the content CID and a gateway URL that
/// resolves it.
pub struct IpfsResult {
    pub cid: String,
    pub gateway_url: String,
}

/// Uploads `bytes` to IPFS via the configured backend and returns the resulting
/// CID plus a gateway URL.
pub async fn upload(
    cfg: &IpfsConfig,
    bytes: Vec<u8>,
    filename: &str,
    content_type: &str,
) -> Result<IpfsResult> {
    let cid = match cfg.backend {
        IpfsBackendKind::Pinata => upload_pinata(cfg, bytes, filename, content_type).await?,
        IpfsBackendKind::Kubo => upload_kubo(cfg, bytes, filename, content_type).await?,
    };

    let gateway = cfg.gateway.as_deref().unwrap_or(match cfg.backend {
        IpfsBackendKind::Pinata => PINATA_DEFAULT_GATEWAY,
        IpfsBackendKind::Kubo => KUBO_DEFAULT_GATEWAY,
    });

    Ok(IpfsResult {
        gateway_url: build_gateway_url(gateway, &cid),
        cid,
    })
}

fn multipart_form(
    bytes: Vec<u8>,
    filename: &str,
    content_type: &str,
) -> Result<reqwest::multipart::Form> {
    let part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str(content_type)?;
    Ok(reqwest::multipart::Form::new().part("file", part))
}

async fn upload_pinata(
    cfg: &IpfsConfig,
    bytes: Vec<u8>,
    filename: &str,
    content_type: &str,
) -> Result<String> {
    // The env var wins over the config value so the secret can stay out of the
    // TOML file.
    let jwt = std::env::var("MEVLOG_PINATA_JWT")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| cfg.pinata_jwt.clone())
        .ok_or_else(|| {
            eyre!(
                "Pinata IPFS upload needs a JWT: set MEVLOG_PINATA_JWT or ipfs.pinata_jwt in config.toml"
            )
        })?;

    let url = format!("{}/v3/files", cfg.pinata_api.trim_end_matches('/'));
    // `network=public` puts the content on the public IPFS network so any
    // gateway can resolve it (the V3 default is private, gateway-only access).
    let form = multipart_form(bytes, filename, content_type)?
        .text("network", "public")
        .text("name", filename.to_string());

    let res = reqwest::Client::new()
        .post(&url)
        .bearer_auth(jwt)
        .multipart(form)
        .send()
        .await
        .map_err(|e| eyre!("Pinata upload request to '{url}' failed: {e}"))?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("Pinata upload failed ({status}): {body}");
    }

    parse_pinata_cid(&body)
}

async fn upload_kubo(
    cfg: &IpfsConfig,
    bytes: Vec<u8>,
    filename: &str,
    content_type: &str,
) -> Result<String> {
    let url = format!("{}/api/v0/add?pin=true", cfg.kubo_api.trim_end_matches('/'));
    let form = multipart_form(bytes, filename, content_type)?;

    let res = reqwest::Client::new()
        .post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            eyre!("Kubo upload request to '{url}' failed (is `ipfs daemon` running?): {e}")
        })?;

    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    if !status.is_success() {
        bail!("Kubo upload failed ({status}): {body}");
    }

    parse_kubo_cid(&body)
}

/// Pinata's V3 Files API returns `{ "data": { "cid": "<cid>", ... } }`.
fn parse_pinata_cid(body: &str) -> Result<String> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|e| eyre!("Pinata response was not valid JSON: {e} (body: {body})"))?;
    value
        .get("data")
        .and_then(|d| d.get("cid"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("Pinata response missing 'data.cid' (body: {body})"))
}

/// Kubo's `/api/v0/add` streams newline-delimited JSON, one object per file
/// (`{ "Name": .., "Hash": "<cid>", "Size": .. }`). For a single file the CID
/// is on the last non-empty line.
fn parse_kubo_cid(body: &str) -> Result<String> {
    let line = body
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| eyre!("Kubo response was empty"))?;
    let value: serde_json::Value = serde_json::from_str(line)
        .map_err(|e| eyre!("Kubo response line was not valid JSON: {e} (line: {line})"))?;
    value
        .get("Hash")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| eyre!("Kubo response missing 'Hash' (line: {line})"))
}

fn build_gateway_url(gateway: &str, cid: &str) -> String {
    format!("{}/ipfs/{}", gateway.trim_end_matches('/'), cid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pinata_cid() {
        let body =
            r#"{"data":{"id":"01","name":"x.html","cid":"bafyabc","size":123,"network":"public"}}"#;
        assert_eq!(parse_pinata_cid(body).unwrap(), "bafyabc");
    }

    #[test]
    fn pinata_missing_hash_errors() {
        // Legacy shape (top-level IpfsHash) is no longer accepted.
        assert!(parse_pinata_cid(r#"{"IpfsHash":"bafyabc"}"#).is_err());
        assert!(parse_pinata_cid(r#"{"data":{"id":"01"}}"#).is_err());
        assert!(parse_pinata_cid("not json").is_err());
    }

    #[test]
    fn parses_kubo_cid_from_last_line() {
        // Multiple files stream one object per line; the wrapping entry is last.
        let body = "{\"Name\":\"a\",\"Hash\":\"Qm111\",\"Size\":\"1\"}\n{\"Name\":\"root\",\"Hash\":\"Qmroot\",\"Size\":\"2\"}\n";
        assert_eq!(parse_kubo_cid(body).unwrap(), "Qmroot");
    }

    #[test]
    fn kubo_empty_or_bad_errors() {
        assert!(parse_kubo_cid("").is_err());
        assert!(parse_kubo_cid("{\"Name\":\"a\"}").is_err());
    }

    #[test]
    fn builds_gateway_url_without_double_slash() {
        assert_eq!(
            build_gateway_url("https://ipfs.io/", "bafy"),
            "https://ipfs.io/ipfs/bafy"
        );
        assert_eq!(
            build_gateway_url("https://gateway.pinata.cloud", "Qm1"),
            "https://gateway.pinata.cloud/ipfs/Qm1"
        );
    }
}
