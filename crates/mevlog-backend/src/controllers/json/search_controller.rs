use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::{
    db::txs::{self, info::db_info},
    misc::shared_init::mevlog_cmd_path,
};
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
    // The scheduler keeps the store indexed; web queries read it as-is.
    cmd.arg("query")
        .arg("--skip-index")
        .arg("--format")
        .arg("json")
        .arg("--rpc-timeout-ms")
        .arg("500")
        .arg("--max-rows")
        .arg("200");
    cmd.env("RUST_LOG", "off");

    let price = get_price_for_chain_id(chain_id).await;

    if let Ok(Some(price)) = price {
        cmd.arg("--native-token-price").arg(price.to_string());
    }

    // Expand {LATEST_BLOCK()} from the locally indexed head instead of an RPC
    // call. The scheduler keeps the store at chain tip; if the DB is missing the
    // query falls back to fetching the latest block over RPC.
    if let Some(latest_block) = indexed_max_block(chain_id).await {
        cmd.arg("--latest-block").arg(latest_block.to_string());
    }

    // Mainnet queries prefer the dedicated ETH_RPC_URL_REMOTE endpoint.
    let remote_rpc_url = std::env::var("ETH_RPC_URL_REMOTE")
        .ok()
        .filter(|url| chain_id == 1 && !url.trim().is_empty());

    if let Some(rpc_url) = remote_rpc_url {
        cmd.arg("--rpc-url").arg(&rpc_url);
    } else if let Ok(Some(rpc_url)) = get_random_rpc_url(chain_id).await {
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

/// Highest block indexed in the local per-chain txs store, read directly from
/// SQLite. Returns `None` (and lets the query fall back to an RPC lookup) when
/// the DB is absent or unreadable.
async fn indexed_max_block(chain_id: u64) -> Option<u64> {
    let db_path = txs::resolve_db_path(None, chain_id);
    if !db_path.exists() {
        return None;
    }

    let conn = txs::conn(Some(db_path.to_string_lossy().into_owned()), chain_id, true)
        .await
        .ok()?;
    let max_block = db_info(&conn).await.ok()?.max_block;
    conn.close().await;
    max_block
}
