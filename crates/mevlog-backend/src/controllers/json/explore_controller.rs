use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use serde::Deserialize;
use tokio::process::Command as AsyncCommand;

use crate::{
    controllers::json::base_controller::{call_json_command_first_line, extract_json_query_params},
    misc::{
        prices::get_price_for_chain_id,
        rpc_utils::get_random_rpc_url,
        utils::{measure_end, measure_start},
    },
};

#[derive(Debug, Deserialize)]
pub struct ExploreParams {
    pub chain_id: Option<u64>,
    #[serde(default)]
    pub block_number: Option<String>,
}

#[hotpath::measure]
pub async fn explore(
    query: Result<Query<ExploreParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    tracing::debug!("params: {:?}", params);

    let chain_id = params.chain_id.unwrap_or(1);

    let mut cmd = AsyncCommand::new("mevlog");
    cmd.arg("search")
        .arg("-b")
        .arg(
            params
                .block_number
                .map_or("latest".to_string(), |bn| bn.to_string()),
        )
        .arg("--format")
        .arg("json")
        .arg("--rpc-timeout-ms")
        .arg("500")
        .arg("--latest-offset") // Improves caching
        .arg("1");
    cmd.env("RUST_LOG", "off");

    let price = get_price_for_chain_id(chain_id).await;

    if let Ok(Some(price)) = price {
        cmd.arg("--native-token-price").arg(price.to_string());
    }

    if let Ok(Some(rpc_url)) = get_random_rpc_url(chain_id).await {
        cmd.arg("--rpc-url").arg(&rpc_url);
    }

    cmd.arg("--chain-id").arg(chain_id.to_string());
    cmd.arg("--skip-verify-chain-id");

    tracing::debug!("explore command: {:?}", &cmd);

    let start = measure_start("explore cmd");
    match call_json_command_first_line::<serde_json::Value>(&mut cmd).await {
        Ok(explore_data) => {
            measure_end(start);
            (StatusCode::OK, Json(explore_data)).into_response()
        }
        Err(error_json) => (StatusCode::BAD_REQUEST, Json(error_json)).into_response(),
    }
}
