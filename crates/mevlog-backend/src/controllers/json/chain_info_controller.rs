use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::ChainInfoNoRpcsJson;
use serde::Deserialize;
use tokio::process::Command as AsyncCommand;

use crate::controllers::json::base_controller::{
    call_json_command_first_line, extract_json_query_params,
};

#[derive(Debug, Deserialize)]
pub struct ChainInfoParams {
    pub chain_id: u64,
}

#[hotpath::measure(log = true)]
pub async fn fetch_chain_info_no_rpcs(chain_id: u64) -> Result<ChainInfoNoRpcsJson, String> {
    let mut cmd = AsyncCommand::new("mevlog");
    cmd.arg("chain-info")
        .arg("--chain-id")
        .arg(chain_id.to_string())
        .arg("--format")
        .arg("json")
        .arg("--skip-urls");
    cmd.env("RUST_LOG", "off");

    match call_json_command_first_line::<ChainInfoNoRpcsJson>(&mut cmd).await {
        Ok(chain_info) => Ok(chain_info),
        Err(e) => Err(format!(
            "Failed to get chain info for chain_id {chain_id}: {e}",
        )),
    }
}

#[hotpath::measure(log = true)]
pub async fn chain_info(
    query: Result<Query<ChainInfoParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    tracing::debug!("params: {:?}", params);

    match fetch_chain_info_no_rpcs(params.chain_id).await {
        Ok(chain_info) => (StatusCode::OK, Json(chain_info)).into_response(),
        Err(error_json) => (StatusCode::BAD_REQUEST, Json(error_json)).into_response(),
    }
}
