use crate::controllers::html::search_controller::SearchParams;
use crate::controllers::{
    base_controller::get_default_blocks,
    websocket::base_controller::{cmd_output_stream, stream_output_lines},
};
use crate::misc::rpc_utils::get_random_rpc_url;
use axum::{
    extract::{
        Query,
        ws::{WebSocket, WebSocketUpgrade},
    },
    http::HeaderMap,
    response::IntoResponse,
};

use futures::stream::StreamExt;
use tokio::process::Command;

#[hotpath::measure]
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<SearchParams>,
    headers: HeaderMap,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, params, headers))
}

#[hotpath::measure]
async fn handle_socket(socket: WebSocket, params: SearchParams, _headers: HeaderMap) {
    let (sender, _receiver) = socket.split();

    let chain_id = params.chain_id.unwrap_or(1);

    let mut cmd = Command::new("mevlog");

    cmd.arg("search")
        .arg("--format")
        .arg("json-stream")
        .arg("--latest-offset") // Improves caching
        .arg("1")
        .arg("--batch-size")
        .arg("20")
        .arg("--max-range")
        .arg("500");

    match get_random_rpc_url(chain_id).await {
        Ok(Some(rpc_url)) => {
            cmd.arg("--rpc-url").arg(&rpc_url);
        }
        _ => {
            cmd.arg("--chain-id").arg(chain_id.to_string());
        }
    }

    let blocks = get_default_blocks(params.blocks.clone());

    cmd.arg("-b").arg(blocks);

    if let Some(position) = params.position.clone() {
        cmd.arg("-p").arg(position);
    }

    if let Some(from) = params.from.clone() {
        cmd.arg("--from").arg(from);
    }

    if let Some(to) = params.to.clone() {
        cmd.arg("--to").arg(to);
    }

    if let Some(event) = params.event.clone() {
        cmd.arg("--event").arg(event);
    }

    if let Some(not_event) = params.not_event.clone() {
        cmd.arg("--not-event").arg(not_event);
    }

    if let Some(method) = params.method.clone() {
        cmd.arg("--method").arg(method);
    }

    if let Some(erc20_transfer) = params.erc20_transfer.clone() {
        cmd.arg("--erc20-transfer").arg(erc20_transfer);
    }

    if let Some(tx_cost) = params.tx_cost.clone() {
        cmd.arg("--tx-cost").arg(tx_cost);
    }

    if let Some(gas_price) = params.gas_price.clone() {
        cmd.arg("--gas-price").arg(gas_price);
    }

    cmd.env("RUST_LOG", "off");

    let (stdout_lines, stderr_lines) = cmd_output_stream(&mut cmd);

    stream_output_lines(stdout_lines, stderr_lines, sender).await;

    tracing::info!("WebSocket connection closed");
}
