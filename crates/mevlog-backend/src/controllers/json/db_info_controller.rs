use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::misc::shared_init::mevlog_cmd_path;
use mevlog::models::json::db_info_response::DbInfoResponse;
use serde::{Deserialize, Serialize};
use tokio::process::Command as AsyncCommand;

use crate::controllers::json::base_controller::{
    call_json_command_first_line, extract_json_query_params,
};

#[derive(Debug, Deserialize)]
pub struct DbInfoParams {
    pub chain_id: Option<u64>,
}

/// Slim subset of the `db-info` command output exposed to the web UI:
/// just the indexed block range and its timestamps.
#[derive(Debug, Serialize)]
struct DbInfoRangeJson {
    min_block: Option<u64>,
    max_block: Option<u64>,
    min_block_timestamp: Option<u64>,
    max_block_timestamp: Option<u64>,
}

#[hotpath::measure(log = true)]
pub(crate) async fn db_info(
    query: Result<Query<DbInfoParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    tracing::debug!("params: {:?}", params);

    let chain_id = params.chain_id.unwrap_or(1);

    let mut cmd = AsyncCommand::new(mevlog_cmd_path());
    cmd.arg("db-info")
        .arg("--chain-id")
        .arg(chain_id.to_string())
        .arg("--format")
        .arg("json");
    cmd.env("RUST_LOG", "off");

    match call_json_command_first_line::<DbInfoResponse>(&mut cmd).await {
        Ok(info) => {
            let range = DbInfoRangeJson {
                min_block: info.min_block,
                max_block: info.max_block,
                min_block_timestamp: info.min_block_timestamp,
                max_block_timestamp: info.max_block_timestamp,
            };
            (StatusCode::OK, Json(range)).into_response()
        }
        Err(error_json) => (StatusCode::BAD_REQUEST, Json(error_json)).into_response(),
    }
}
