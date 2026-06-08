use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use serde::Deserialize;

use crate::controllers::json::base_controller::{block_txs_with_logs, extract_json_query_params};

#[derive(Debug, Deserialize)]
pub struct ExploreParams {
    pub chain_id: Option<u64>,
    #[serde(default)]
    pub block_number: Option<String>,
}

#[hotpath::measure]
pub(crate) async fn explore(
    query: Result<Query<ExploreParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    tracing::debug!("params: {:?}", params);

    let chain_id = params.chain_id.unwrap_or(1);

    match block_txs_with_logs(chain_id, params.block_number).await {
        Ok(explore_data) => (StatusCode::OK, Json(explore_data)).into_response(),
        Err(error_json) => (StatusCode::BAD_REQUEST, Json(error_json)).into_response(),
    }
}
