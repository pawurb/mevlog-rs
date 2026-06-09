use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::misc::shared_init::mevlog_cmd_path;
use tokio::process::Command as AsyncCommand;

use crate::{
    controllers::{
        html::search_controller::SearchParams,
        json::base_controller::{call_json_command_first_line, extract_json_query_params},
    },
    misc::{prices::get_price_for_chain_id, rpc_utils::get_random_rpc_url},
};

#[hotpath::measure]
pub(crate) async fn search(
    query: Result<Query<SearchParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    tracing::debug!("params: {:?}", params);

    let chain_id = params.chain_id.unwrap_or(1);

    let mut cmd = AsyncCommand::new(mevlog_cmd_path());
    // Block range is hardcoded: not settable via the web API.
    cmd.arg("query")
        .arg("-b")
        .arg("latest")
        .arg("--format")
        .arg("json")
        .arg("--rpc-timeout-ms")
        .arg("500")
        .arg("--latest-offset")
        .arg("2")
        .arg("--batch-size")
        .arg("20")
        .arg("--max-range")
        .arg("500");
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

    if let Some(sql) = params.sql {
        cmd.arg("--sql").arg(sql);
    }

    tracing::debug!("search command: {:?}", &cmd);

    match call_json_command_first_line::<serde_json::Value>(&mut cmd).await {
        Ok(search_data) => (StatusCode::OK, Json(search_data)).into_response(),
        Err(error_json) => (StatusCode::BAD_REQUEST, Json(error_json)).into_response(),
    }
}
