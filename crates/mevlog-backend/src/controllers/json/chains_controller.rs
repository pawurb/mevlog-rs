use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::ChainEntryJson;
use serde::Deserialize;
use tokio::process::Command as AsyncCommand;

use crate::controllers::json::base_controller::{
    call_json_command_first_line, extract_json_query_params,
};

#[derive(Debug, Deserialize)]
pub struct ChainsParams {
    pub filter: Option<String>,
    pub limit: Option<u64>,
    pub chain_id: Option<u64>,
}

#[hotpath::measure(log = true)]
pub async fn chains(
    query: Result<Query<ChainsParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    let mut cmd = AsyncCommand::new("mevlog");
    cmd.arg("chains").arg("--format").arg("json");
    cmd.env("RUST_LOG", "off");

    if let Some(filter) = &params.filter {
        cmd.arg("--filter").arg(filter);
    }

    if let Some(limit) = params.limit {
        cmd.arg("--limit").arg(limit.to_string());
    }

    if let Some(chain_id) = &params.chain_id {
        cmd.arg("--chain-id").arg(chain_id.to_string());
    } else if params.filter.is_none() && params.limit.is_none() {
        // If no parameters are provided, return default popular chains
        let default_chain_ids = [1, 137, 8453, 10, 130, 43114, 56, 42161];
        for chain_id in default_chain_ids {
            cmd.arg("--chain-id").arg(chain_id.to_string());
        }
    }

    match call_json_command_first_line::<Vec<ChainEntryJson>>(&mut cmd).await {
        Ok(chains) => (StatusCode::OK, Json(chains)).into_response(),
        Err(error_json) => (StatusCode::BAD_REQUEST, Json(error_json)).into_response(),
    }
}
