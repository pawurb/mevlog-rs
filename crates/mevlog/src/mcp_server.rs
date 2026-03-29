use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use rmcp::{
    ErrorData as McpError, ServerHandler,
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router,
    transport::streamable_http_server::{
        StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
    },
};
use schemars::JsonSchema;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info};

#[derive(Debug, Deserialize, JsonSchema)]
struct GetTransactionParams {
    #[schemars(description = "Transaction hash (0x-prefixed hex string)")]
    tx_hash: String,
    #[schemars(
        description = "Tracing mode: 'revm' for local simulation or 'rpc' for debug_traceTransaction"
    )]
    evm_trace: Option<String>,
    #[schemars(description = "Number of transactions before (newer, smaller indexes) to include")]
    before: Option<u8>,
    #[schemars(description = "Number of transactions after (older, larger indexes) to include")]
    after: Option<u8>,
    #[schemars(description = "Reverse the order of transactions")]
    reverse: Option<bool>,
    #[schemars(description = "Show detailed tx calls info (requires evm_trace)")]
    evm_calls: Option<bool>,
    #[schemars(
        description = "Display EVM opcodes executed by the transaction (requires evm_trace)"
    )]
    evm_ops: Option<bool>,
    #[schemars(
        description = "Display storage slot changes (state diff) for the transaction (requires evm_trace)"
    )]
    evm_state_diff: Option<bool>,
    #[schemars(description = "Display amounts in ERC20 Transfer event logs")]
    erc20_transfer_amount: Option<bool>,
    #[schemars(description = "Enable ENS name resolution for addresses")]
    ens: Option<bool>,
    #[schemars(description = "Enable ERC20 token symbol resolution")]
    erc20_symbols: Option<bool>,
    #[schemars(description = "Include event logs in output")]
    logs: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct SearchTransactionsParams {
    #[schemars(
        description = "Block range: 'latest', a block number like '22030899', or a range like '22030800:22030900'"
    )]
    blocks: String,
    #[schemars(description = "Filter by sender address (0x-prefixed) or ENS name")]
    from: Option<String>,
    #[schemars(
        description = "Filter by recipient address (0x-prefixed), ENS name, or 'CREATE' for contract creations"
    )]
    to: Option<String>,
    #[schemars(
        description = "Filter by event signature name or regex (e.g., 'Transfer' or '/Swap/')"
    )]
    event: Option<String>,
    #[schemars(
        description = "Exclude txs by event names matching the provided regex or signature"
    )]
    not_event: Option<String>,
    #[schemars(description = "Filter by root method signature name or regex")]
    method: Option<String>,
    #[schemars(
        description = "Filter by subcall method names matching the provided regex, signature or signature hash (requires evm_trace)"
    )]
    calls: Option<String>,
    #[schemars(
        description = "Filter by contracts with storage changed by the transaction (0x-prefixed address, requires evm_trace)"
    )]
    touching: Option<String>,
    #[schemars(description = "Maximum number of transactions to return")]
    limit: Option<u32>,
    #[schemars(description = "Tracing mode: 'revm' or 'rpc'")]
    evm_trace: Option<String>,
    #[schemars(description = "If true, only return failed transactions")]
    failed: Option<bool>,
    #[schemars(
        description = "Sort transactions by field: 'gas-price', 'gas-used', 'tx-cost', 'full-tx-cost', or 'erc20Transfer|<token_address>'"
    )]
    sort: Option<String>,
    #[schemars(description = "Sort direction: 'asc' or 'desc' (default: 'desc')")]
    sort_dir: Option<String>,
    #[schemars(
        description = "Transaction position or range within a block (e.g., '0' for first tx, '0:10' for first 11 txs)"
    )]
    position: Option<String>,
    #[schemars(description = "Filter by tx cost (e.g., 'le0.001ether', 'ge0.01ether')")]
    tx_cost: Option<String>,
    #[schemars(
        description = "Filter by real (including coinbase bribe) tx cost (e.g., 'le0.001ether', 'ge0.01ether')"
    )]
    real_tx_cost: Option<String>,
    #[schemars(description = "Filter by effective gas price (e.g., 'ge2gwei', 'le1gwei')")]
    gas_price: Option<String>,
    #[schemars(
        description = "Filter by real (including coinbase bribe) effective gas price (e.g., 'ge3gwei', 'le2gwei')"
    )]
    real_gas_price: Option<String>,
    #[schemars(description = "Filter by transaction value (e.g., 'ge1ether', 'le0.1ether')")]
    value: Option<String>,
    #[schemars(
        description = "Filter by ERC20 Transfer events with address and optionally amount (e.g., '0xa0b8...|ge1000gwei')"
    )]
    erc20_transfer: Option<String>,
    #[schemars(description = "Show detailed tx calls info (requires evm_trace)")]
    evm_calls: Option<bool>,
    #[schemars(
        description = "Display EVM opcodes executed by the transaction (requires evm_trace)"
    )]
    evm_ops: Option<bool>,
    #[schemars(
        description = "Display storage slot changes (state diff) for the transaction (requires evm_trace)"
    )]
    evm_state_diff: Option<bool>,
    #[schemars(description = "Display amounts in ERC20 Transfer event logs")]
    erc20_transfer_amount: Option<bool>,
    #[schemars(description = "Enable ENS name resolution for addresses")]
    ens: Option<bool>,
    #[schemars(description = "Enable ERC20 token symbol resolution")]
    erc20_symbols: Option<bool>,
    #[schemars(description = "Include event logs in output")]
    logs: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListChainsParams {
    #[schemars(description = "Filter chains by name (case-insensitive substring match)")]
    filter: Option<String>,
    #[schemars(description = "Maximum number of chains to return")]
    limit: Option<u32>,
    #[schemars(description = "Comma-separated chain IDs to filter by")]
    chain_ids: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ChainInfoParams {
    #[schemars(description = "Chain ID to get information for")]
    chain_id: u64,
}

#[derive(Clone)]
pub struct MevlogMcpServer {
    tool_router: ToolRouter<Self>,
    rpc_url: String,
    chain_id: u64,
}

#[tool_router]
impl MevlogMcpServer {
    fn new(rpc_url: String, chain_id: u64) -> Self {
        Self {
            tool_router: Self::tool_router(),
            rpc_url,
            chain_id,
        }
    }

    #[tool(
        description = r#"Get detailed information about a specific Ethereum transaction.

Returns JSON with transaction details including: hash, block number, from/to addresses, value, gas usage, method signature, event logs, and optionally traced call details.

Use the 'evm_trace' parameter with 'revm' (local EVM simulation) or 'rpc' (debug_traceTransaction) to get internal calls and state changes."#
    )]
    async fn get_transaction(
        &self,
        params: Parameters<GetTransactionParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(tx_hash = %params.0.tx_hash, evm_trace = ?params.0.evm_trace, "MCP get_transaction request");
        let mut args = vec!["tx".to_string(), params.0.tx_hash];
        if let Some(evm_trace) = params.0.evm_trace {
            args.push("--evm-trace".to_string());
            args.push(evm_trace);
        }
        if let Some(before) = params.0.before {
            args.push("--before".to_string());
            args.push(before.to_string());
        }
        if let Some(after) = params.0.after {
            args.push("--after".to_string());
            args.push(after.to_string());
        }
        if params.0.reverse == Some(true) {
            args.push("--reverse".to_string());
        }
        if params.0.evm_calls == Some(true) {
            args.push("--evm-calls".to_string());
        }
        if params.0.evm_ops == Some(true) {
            args.push("--evm-ops".to_string());
        }
        if params.0.evm_state_diff == Some(true) {
            args.push("--evm-state-diff".to_string());
        }
        if params.0.erc20_transfer_amount == Some(true) {
            args.push("--erc20-transfer-amount".to_string());
        }
        if params.0.ens == Some(true) {
            args.push("--ens".to_string());
        }
        if params.0.erc20_symbols == Some(true) {
            args.push("--erc20-symbols".to_string());
        }
        if let Some(logs) = params.0.logs {
            args.push("--logs".to_string());
            args.push(logs.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Search for Ethereum transactions matching filter conditions within a block range.

Returns JSON array of matching transactions. Supports filtering by sender, recipient, event signatures, method signatures, and failed status.

Block range examples: 'latest' (latest block), '22030899' (single block), '22030800:22030900' (range), '50:latest' (last 50 blocks)."#
    )]
    async fn search_transactions(
        &self,
        params: Parameters<SearchTransactionsParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(
            blocks = %params.0.blocks,
            from = ?params.0.from,
            to = ?params.0.to,
            event = ?params.0.event,
            method = ?params.0.method,
            limit = ?params.0.limit,
            evm_trace = ?params.0.evm_trace,
            failed = ?params.0.failed,
            sort = ?params.0.sort,
            sort_dir = ?params.0.sort_dir,
            position = ?params.0.position,
            "MCP search_transactions request"
        );
        let mut args = vec!["search".to_string(), "-b".to_string(), params.0.blocks];
        if let Some(from) = params.0.from {
            args.push("--from".to_string());
            args.push(from);
        }
        if let Some(to) = params.0.to {
            args.push("--to".to_string());
            args.push(to);
        }
        if let Some(event) = params.0.event {
            args.push("--event".to_string());
            args.push(event);
        }
        if let Some(not_event) = params.0.not_event {
            args.push("--not-event".to_string());
            args.push(not_event);
        }
        if let Some(method) = params.0.method {
            args.push("--method".to_string());
            args.push(method);
        }
        if let Some(calls) = params.0.calls {
            args.push("--calls".to_string());
            args.push(calls);
        }
        if let Some(touching) = params.0.touching {
            args.push("--touching".to_string());
            args.push(touching);
        }
        if let Some(limit) = params.0.limit {
            args.push("--limit".to_string());
            args.push(limit.to_string());
        }
        if let Some(evm_trace) = params.0.evm_trace {
            args.push("--evm-trace".to_string());
            args.push(evm_trace);
        }
        if params.0.failed == Some(true) {
            args.push("--failed".to_string());
        }
        if let Some(sort) = params.0.sort {
            args.push("--sort".to_string());
            args.push(sort);
        }
        if let Some(sort_dir) = params.0.sort_dir {
            args.push("--sort-dir".to_string());
            args.push(sort_dir);
        }
        if let Some(position) = params.0.position {
            args.push("--position".to_string());
            args.push(position);
        }
        if let Some(tx_cost) = params.0.tx_cost {
            args.push("--tx-cost".to_string());
            args.push(tx_cost);
        }
        if let Some(real_tx_cost) = params.0.real_tx_cost {
            args.push("--real-tx-cost".to_string());
            args.push(real_tx_cost);
        }
        if let Some(gas_price) = params.0.gas_price {
            args.push("--gas-price".to_string());
            args.push(gas_price);
        }
        if let Some(real_gas_price) = params.0.real_gas_price {
            args.push("--real-gas-price".to_string());
            args.push(real_gas_price);
        }
        if let Some(value) = params.0.value {
            args.push("--value".to_string());
            args.push(value);
        }
        if let Some(erc20_transfer) = params.0.erc20_transfer {
            args.push("--erc20-transfer".to_string());
            args.push(erc20_transfer);
        }
        if params.0.evm_calls == Some(true) {
            args.push("--evm-calls".to_string());
        }
        if params.0.evm_ops == Some(true) {
            args.push("--evm-ops".to_string());
        }
        if params.0.evm_state_diff == Some(true) {
            args.push("--evm-state-diff".to_string());
        }
        if params.0.erc20_transfer_amount == Some(true) {
            args.push("--erc20-transfer-amount".to_string());
        }
        if params.0.ens == Some(true) {
            args.push("--ens".to_string());
        }
        if params.0.erc20_symbols == Some(true) {
            args.push("--erc20-symbols".to_string());
        }
        if let Some(logs) = params.0.logs {
            args.push("--logs".to_string());
            args.push(logs.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = r#"List all available EVM chains from ChainList.

Returns JSON array of chains with chain_id, name, and explorer URL. Use filters to narrow results."#)]
    async fn list_chains(
        &self,
        params: Parameters<ListChainsParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(
            filter = ?params.0.filter,
            limit = ?params.0.limit,
            chain_ids = ?params.0.chain_ids,
            "MCP list_chains request"
        );
        let mut args = vec!["chains".to_string()];
        if let Some(filter) = params.0.filter {
            args.push("--filter".to_string());
            args.push(filter);
        }
        if let Some(limit) = params.0.limit {
            args.push("--limit".to_string());
            args.push(limit.to_string());
        }
        if let Some(chain_ids) = params.0.chain_ids {
            for id in chain_ids.split(',') {
                let id = id.trim();
                if !id.is_empty() {
                    args.push("--chain-id".to_string());
                    args.push(id.to_string());
                }
            }
        }
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = r#"Get detailed information about a specific EVM chain.

Returns JSON with chain_id, name, currency symbol, and explorer URL."#)]
    async fn chain_info(
        &self,
        params: Parameters<ChainInfoParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(chain_id = params.0.chain_id, "MCP chain_info request");
        let args = vec![
            "chain-info".to_string(),
            "--chain-id".to_string(),
            params.0.chain_id.to_string(),
            "--skip-urls".to_string(),
        ];
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Check if the configured RPC endpoint supports debug tracing (debug_traceTransaction).

Returns 'true' or 'false'. Debug tracing is required for features like --evm-trace rpc, internal call tracing, and state diffs."#
    )]
    async fn check_debug_available(&self) -> Result<CallToolResult, McpError> {
        debug!("MCP check_debug_available request");
        let args = vec![
            "debug-available".to_string(),
            "--rpc-url".to_string(),
            self.rpc_url.clone(),
        ];
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

impl MevlogMcpServer {
    fn push_conn_args(&self, args: &mut Vec<String>) {
        args.push("--rpc-url".to_string());
        args.push(self.rpc_url.clone());
        args.push("--chain-id".to_string());
        args.push(self.chain_id.to_string());
    }

    fn build_cli_args(&self, args: &[String]) -> Vec<String> {
        let mut cli_args = vec!["--format".to_string(), "json".to_string()];
        cli_args.extend_from_slice(args);
        cli_args
    }

    async fn run_mevlog_cmd(&self, args: &[String]) -> Result<String, McpError> {
        let cli_args = self.build_cli_args(args);
        let logged: Vec<_> = {
            let mut out = Vec::new();
            let mut skip_next = false;
            for a in &cli_args {
                if skip_next {
                    skip_next = false;
                    continue;
                }
                if a == "--rpc-url" {
                    skip_next = true;
                    continue;
                }
                out.push(a.as_str());
            }
            out
        };
        debug!(command = %logged.join(" "), "executing mevlog CLI for MCP request");

        let mut cmd = tokio::process::Command::new(crate::misc::shared_init::mevlog_cmd_path());
        cmd.args(&cli_args);

        let output = cmd.output().await.map_err(|e| {
            McpError::internal_error(format!("Failed to execute mevlog: {e}"), None)
        })?;

        if output.status.success() {
            debug!(
                stdout_bytes = output.stdout.len(),
                "mevlog CLI finished successfully"
            );
            String::from_utf8(output.stdout).map_err(|e| {
                McpError::internal_error(format!("Invalid UTF-8 in output: {e}"), None)
            })
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(
                status = ?output.status.code(),
                stderr = %stderr.trim(),
                "mevlog CLI failed during MCP request"
            );
            Err(McpError::internal_error(
                format!("mevlog exited with error: {stderr}"),
                None,
            ))
        }
    }
}

#[tool_handler]
impl ServerHandler for MevlogMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "mevlog".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: None,
                website_url: None,
                icons: None,
            },
            instructions: Some(
                "mevlog MCP server. Provides tools for Ethereum transaction analysis, searching, and chain information."
                    .into(),
            ),
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

fn bearer_token(header: &str) -> Option<&str> {
    let (scheme, token) = header.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("bearer") {
        return None;
    }

    let token = token.trim();
    if token.is_empty() { None } else { Some(token) }
}

fn check_auth(expected: Option<&str>, provided: Option<&str>) -> bool {
    match expected {
        None => true,
        Some(expected) => provided
            .map(|header| {
                bearer_token(header)
                    .map(|token| constant_time_eq(token.as_bytes(), expected.as_bytes()))
                    .unwrap_or(false)
            })
            .unwrap_or(false),
    }
}

async fn auth_middleware(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::http::StatusCode> {
    let expected = std::env::var("MEVLOG_MCP_AUTH_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());
    let provided = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    if check_auth(expected.as_deref(), provided) {
        Ok(next.run(request).await)
    } else {
        Err(axum::http::StatusCode::UNAUTHORIZED)
    }
}

pub async fn run_mcp_server(rpc_url: String, chain_id: u64, port: u16) -> eyre::Result<()> {
    let cancellation_token = CancellationToken::new();
    let server_rpc_url = rpc_url.clone();

    let config = StreamableHttpServerConfig {
        sse_keep_alive: Some(Duration::from_secs(15)),
        sse_retry: None,
        stateful_mode: true,
        cancellation_token: cancellation_token.clone(),
    };

    let service = StreamableHttpService::new(
        move || Ok(MevlogMcpServer::new(server_rpc_url.clone(), chain_id)),
        Arc::new(LocalSessionManager::default()),
        config,
    );

    let app = Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(auth_middleware));

    let addr = format!("localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!(
        port,
        chain_id, "mevlog MCP server listening on http://{addr}/mcp"
    );

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            cancellation_token.cancelled().await;
        })
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::mcp_server::{bearer_token, check_auth, constant_time_eq};

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"short", b"longer"));
    }

    #[test]
    fn bearer_token_extracts_common_formats() {
        assert_eq!(bearer_token("Bearer secret"), Some("secret"));
        assert_eq!(bearer_token("bearer secret"), Some("secret"));
        assert_eq!(bearer_token("Bearer   secret  "), Some("secret"));
        assert_eq!(bearer_token("Basic secret"), None);
        assert_eq!(bearer_token("Bearer"), None);
    }

    #[test]
    fn auth_disabled_allows_all() {
        assert!(check_auth(None, None));
        assert!(check_auth(None, Some("anything")));
    }

    #[test]
    fn auth_enabled_rejects_missing() {
        assert!(!check_auth(Some("secret"), None));
    }

    #[test]
    fn auth_enabled_rejects_wrong() {
        assert!(!check_auth(Some("secret"), Some("wrong")));
        assert!(!check_auth(Some("secret"), Some("Secret")));
        assert!(!check_auth(Some("secret"), Some("")));
        assert!(!check_auth(Some("secret"), Some("secret")));
        assert!(!check_auth(Some("secret"), Some("Bearer wrong")));
    }

    #[test]
    fn auth_enabled_accepts_correct() {
        assert!(check_auth(Some("secret"), Some("Bearer secret")));
        assert!(check_auth(Some("secret"), Some("bearer secret")));
        assert!(!check_auth(Some("Bearer token"), Some("Bearer token")));
    }

    #[test]
    fn build_cli_args_keeps_conn_flags_after_subcommand_args() {
        let server = crate::mcp_server::MevlogMcpServer::new("http://localhost:8545".into(), 1);
        let mut args = vec![
            "search".to_string(),
            "-b".to_string(),
            "10:latest".to_string(),
        ];

        server.push_conn_args(&mut args);

        assert_eq!(
            server.build_cli_args(&args),
            vec![
                "--format".to_string(),
                "json".to_string(),
                "search".to_string(),
                "-b".to_string(),
                "10:latest".to_string(),
                "--rpc-url".to_string(),
                "http://localhost:8545".to_string(),
                "--chain-id".to_string(),
                "1".to_string(),
            ]
        );
    }
}
