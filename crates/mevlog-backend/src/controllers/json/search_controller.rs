use axum::{Json, extract::Query, http::StatusCode, response::IntoResponse};
use mevlog::{
    cmds::query::query,
    db::txs::{self, info::db_info},
    misc::shared_init::{ConnOpts, CryoOpts, SharedOpts},
    models::json::query_response::serialize_query_response,
};

use crate::{
    controllers::{
        base_controller::decorate_error_message, html::search_controller::SearchParams,
        json::base_controller::extract_json_query_params,
    },
    misc::{prices::get_price_for_chain_id, rpc_utils::get_random_rpc_url},
};

// Kept below the server-wide 10s TimeoutLayer so query()'s decorated timeout
// error wins the race instead of the outer layer's generic 408.
const QUERY_TIMEOUT_MS: u64 = 8000;

#[hotpath::measure]
pub(crate) async fn search(
    query_params: Result<Query<SearchParams>, axum::extract::rejection::QueryRejection>,
) -> impl IntoResponse {
    let params = match extract_json_query_params(query_params) {
        Ok(params) => params,
        Err(error_response) => return error_response.into_response(),
    };

    tracing::debug!("params: {:?}", params);

    let chain_id = params.chain_id.unwrap_or(1);

    let native_token_price = get_price_for_chain_id(chain_id).await.ok().flatten();

    // Expand {LATEST_BLOCK()} from the locally indexed head instead of an RPC
    // call. The scheduler keeps the store at chain tip; if the DB is missing the
    // query falls back to fetching the latest block over RPC.
    let latest_block = indexed_max_block(chain_id).await;

    // Mainnet queries prefer the dedicated ETH_RPC_URL_REMOTE endpoint.
    let remote_rpc_url = std::env::var("ETH_RPC_URL_REMOTE")
        .ok()
        .filter(|url| chain_id == 1 && !url.trim().is_empty());

    let rpc_url = match remote_rpc_url {
        Some(rpc_url) => Some(rpc_url),
        None => get_random_rpc_url(chain_id).await.ok().flatten(),
    };

    let conn_opts = ConnOpts {
        rpc_url: rpc_url.into_iter().collect(),
        chain_id: Some(chain_id),
        rpc_timeout_ms: 500,
        block_timeout_ms: 10000,
        skip_verify_chain_id: true,
        txs_db_dir: None,
    };

    let shared_opts = SharedOpts {
        evm_trace: None,
        native_token_price,
    };

    let cryo_opts = CryoOpts::default();

    let sql = params.sql.unwrap_or_default();

    // The scheduler keeps the store indexed; web queries read it as-is
    // (skip_index = true => no block range resolution, fetching, or backfill).
    // The timeout is enforced inside query() (shared with the CLI's --timeout-ms).
    let outcome = match query(
        None, // blocks
        None, // latest_offset
        None, // max_range
        Some(200),
        100,  // batch_size (CLI default)
        true, // skip_index
        latest_block,
        &sql,
        &shared_opts,
        &conn_opts,
        &cryo_opts,
        Some(QUERY_TIMEOUT_MS),
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            let message = decorate_error_message(&error.to_string());
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": message })),
            )
                .into_response();
        }
    };

    let body = match serialize_query_response(
        outcome.rows,
        false,
        outcome.chain,
        outcome.duration_ns,
        outcome.cached_blocks,
        outcome.new_blocks,
        outcome.query,
        None,
    ) {
        Ok(body) => body,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response();
        }
    };

    match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(value) => (StatusCode::OK, Json(value)).into_response(),
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response(),
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
