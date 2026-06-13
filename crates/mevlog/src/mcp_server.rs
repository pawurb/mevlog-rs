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

/// Extra time on top of the per-request timeout before the CLI subprocess is
/// force-killed: covers process startup (before the CLI's own clock starts) and
/// lets the CLI's own timeout error surface ahead of this hard kill.
const KILL_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug, Deserialize, JsonSchema)]
// Reject stale clients still sending the removed `blocks`/`skip_index` params
// instead of silently querying the whole DB.
#[serde(deny_unknown_fields)]
struct QueryParams {
    #[schemars(
        description = "Read-only SQL run against the local txs DB. Tables: transactions, logs, blocks. See the tool description for the full schema, U256 helper functions and {MACRO()} reference."
    )]
    sql: String,
    #[schemars(
        description = "Native token price in USD (e.g. 3500.0 for ETH); also feeds the {NATIVE_TOKEN_PRICE()} macro and convert_usd(wei, price)"
    )]
    native_token_price: Option<f64>,
    #[schemars(description = "Maximum number of rows the query may return (errors when exceeded)")]
    max_rows: Option<usize>,
}

#[derive(Clone)]
pub struct MevlogMcpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    rpc_url: String,
    chain_id: u64,
    timeout: Duration,
}

#[tool_router]
impl MevlogMcpServer {
    fn new(rpc_url: String, chain_id: u64, timeout: Duration) -> Self {
        Self {
            tool_router: Self::tool_router(),
            rpc_url,
            chain_id,
            timeout,
        }
    }

    #[tool(
        description = r#"Run a read-only SQL query against the local store of indexed Ethereum transactions.

This is the only tool. It runs `sql` over the per-chain SQLite store as-is (read-only; no indexing or fetching of new blocks) and returns a JSON `QueryResponse` envelope (`result`, `duration`, `chain`, `query` — `query.sql` echoes the fully-substituted SQL that produced `result`). The store is populated out-of-band by the operator (e.g. `mevlog index --live`); this tool never writes to it.

SCHEMA — three tables (exact column names):
  • transactions(block_number, tx_index, tx_hash, nonce, from_address, to_address, value, gas_limit, gas_used, effective_gas_price, gas_price, max_fee_per_gas, max_priority_fee_per_gas, transaction_type, success, coinbase_transfer, signature_hash, signature)
      signature = human-readable method signature TEXT (e.g. 'transfer(address,uint256)'), signature_hash = 4-byte selector BLOB. There is NO `method` column.
  • logs(block_number, tx_index, log_index, address, topic0, topic1, topic2, topic3, data, erc20_amount, signature)
      erc20_amount = decoded ERC20 Transfer amount as a 32-byte big-endian BLOB (NULL for non-transfer logs). signature = human-readable event signature TEXT.
  • blocks(block_number, block_hash, miner, gas_used, timestamp, base_fee_per_gas)

RULES:
  • Address/hash columns are BLOBs, emitted as 0x-hex. In predicates they MUST be blob literals: WHERE from_address = X'1111...1111'.
  • `success` is 0/1 (SQLite has no boolean).
  • Plain SQL SUM() cannot total U256 BLOB columns (value, erc20_amount, gas cost) — use the helper functions below.
  • Never ORDER BY a u256_to_dec()/format_ether()/format_gwei()/format_usd() result — they return TEXT that SQLite sorts lexicographically (so '9' > '10'). Sort on the underlying BLOB/numeric expression and apply the display helper only in the projection.

U256 / display helper functions:
  • u256_sum(col)            aggregate sum of 32-byte BLOB column → 0x-hex BLOB
  • u256_mul(a,b) / u256_add(a,b)   exact 256-bit scalar math → BLOB (e.g. u256_mul(gas_used, effective_gas_price) = tx cost)
  • u256_to_dec(col)         BLOB → full-precision decimal string
  • erc20_to_real(amount, decimals)   amount / 10^decimals → REAL (approx f64), e.g. erc20_to_real(erc20_amount, 6) for USDC
  • format_ether(col) / format_gwei(col)   wei → ETH / gwei display strings
  • convert_usd(wei, price)   wei → USD amount (REAL) at the given native-token price
  • format_usd(amount)   amount → '$'-prefixed USD display string (single arg, NOT format_usd(col, price))
      USD display = format_usd(convert_usd(wei, {NATIVE_TOKEN_PRICE()})). Pass the price to convert_usd, then wrap in format_usd. e.g. format_usd(convert_usd(u256_mul(gas_used, effective_gas_price), {NATIVE_TOKEN_PRICE()})) for tx cost in USD.

MACROS (must be brace-wrapped; resolved over RPC only when present):
  • {LATEST_BLOCK()}        → current latest block number, e.g. WHERE block_number > {LATEST_BLOCK()} - 100
  • {NATIVE_TOKEN_PRICE()}  → native token USD price (from native_token_price param or Chainlink oracle)
  • {RESOLVE_ENS("name.eth")} → resolved address as a X'..' blob literal (Ethereum mainnet only)

Use the {LATEST_BLOCK()} macro (not a block param) to bound a query to a recent window over already-indexed data.

EXAMPLES:
  • Total USDC transferred in the last 100 indexed blocks:
      sql="SELECT u256_sum(erc20_amount) AS total FROM logs WHERE block_number > {LATEST_BLOCK()} - 100 AND address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48' AND erc20_amount IS NOT NULL"
  • Top 10 most expensive transactions in a block (order by the raw cost, NOT by the decimal string — u256_to_dec/format_* produce TEXT that SQLite sorts lexicographically, giving a wrong ranking; sort on a numeric or blob expression and convert only in the projection). Here gas_used*effective_gas_price fits in INTEGER for ordering:
      sql="SELECT tx_hash, u256_to_dec(u256_mul(gas_used, effective_gas_price)) AS cost_wei FROM transactions WHERE block_number = 22030899 ORDER BY gas_used * effective_gas_price DESC LIMIT 10"
  • Transactions from an ENS name in the last 50 indexed blocks:
      sql="SELECT tx_hash, format_ether(value) AS eth FROM transactions WHERE block_number > {LATEST_BLOCK()} - 50 AND from_address = {RESOLVE_ENS(\"vitalik.eth\")}"
  • Failed transactions in a block (decoded method signature is the `signature` column, not `method`):
      sql="SELECT tx_hash, signature FROM transactions WHERE block_number = 22030899 AND success = 0""#
    )]
    async fn query(&self, params: Parameters<QueryParams>) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!("MCP query request");
        // Always --skip-index: this tool is read-only and never fetches or
        // writes blocks. The local store is populated out-of-band by the
        // operator (e.g. `mevlog index --live`).
        let mut args = vec![
            "query".to_string(),
            "--skip-index".to_string(),
            "--timeout-ms".to_string(),
            self.timeout.as_millis().to_string(),
            "--sql".to_string(),
            p.sql,
        ];
        if let Some(max_rows) = p.max_rows {
            args.push("--max-rows".to_string());
            args.push(max_rows.to_string());
        }
        if let Some(price) = p.native_token_price {
            args.push("--native-token-price".to_string());
            args.push(price.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Show read-only stats for the local per-chain transactions database (indexed block range, row counts, file size) for the server's configured chain."#
    )]
    async fn db_info(&self) -> Result<CallToolResult, McpError> {
        debug!("MCP db_info request");
        let args = vec![
            "db-info".to_string(),
            "--chain-id".to_string(),
            self.chain_id.to_string(),
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
        // Kill the spawned process if the timeout future below is dropped.
        cmd.kill_on_drop(true);

        // The request budget (self.timeout) is enforced inside the CLI: `query`
        // gets it via --timeout-ms (covering RPC, indexing and SQL). This is only
        // a hard-kill backstop for cases the CLI can't self-bound — db_info (no
        // internal timeout) or a wedged child. KILL_GRACE covers process startup
        // before the CLI's own clock starts and lets its cleaner error win.
        let backstop = self.timeout + KILL_GRACE;
        let output = match tokio::time::timeout(backstop, cmd.output()).await {
            Ok(res) => res.map_err(|e| {
                McpError::internal_error(format!("Failed to execute mevlog: {e}"), None)
            })?,
            Err(_) => {
                let ms = backstop.as_millis();
                error!(timeout_ms = ms, "mevlog CLI timed out during MCP request");
                return Err(McpError::internal_error(
                    format!("mevlog timed out after {ms}ms"),
                    None,
                ));
            }
        };

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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("mevlog", env!("CARGO_PKG_VERSION")),
            )
            .with_instructions(
                "mevlog MCP server. Exposes two read-only tools: `query` runs SQL against a local store of indexed Ethereum transactions (no indexing or writes), and `db_info` reports the local store's indexed block range, row counts and file size.",
            )
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

pub async fn run_mcp_server(
    rpc_url: String,
    chain_id: u64,
    host: &str,
    port: u16,
    timeout: Duration,
) -> eyre::Result<()> {
    let cancellation_token = CancellationToken::new();
    let server_rpc_url = rpc_url.clone();

    let config = StreamableHttpServerConfig::default()
        .with_sse_keep_alive(Some(Duration::from_secs(15)))
        .with_sse_retry(None)
        .with_cancellation_token(cancellation_token.clone());

    let service = StreamableHttpService::new(
        move || {
            Ok(MevlogMcpServer::new(
                server_rpc_url.clone(),
                chain_id,
                timeout,
            ))
        },
        Arc::new(LocalSessionManager::default()),
        config,
    );

    let app = Router::new()
        .nest_service("/mcp", service)
        .layer(axum::middleware::from_fn(auth_middleware))
        .fallback(|| async {
            (
                axum::http::StatusCode::NOT_FOUND,
                [("content-type", "application/json")],
                r#"{"error":"not_found"}"#,
            )
        });

    let addr = format!("{host}:{port}");
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
        let server = crate::mcp_server::MevlogMcpServer::new(
            "http://localhost:8545".into(),
            1,
            std::time::Duration::from_millis(30000),
        );
        let mut args = vec![
            "query".to_string(),
            "-b".to_string(),
            "10:latest".to_string(),
            "--sql".to_string(),
            "SELECT * FROM transactions".to_string(),
        ];

        server.push_conn_args(&mut args);

        assert_eq!(
            server.build_cli_args(&args),
            vec![
                "--format".to_string(),
                "json".to_string(),
                "query".to_string(),
                "-b".to_string(),
                "10:latest".to_string(),
                "--sql".to_string(),
                "SELECT * FROM transactions".to_string(),
                "--rpc-url".to_string(),
                "http://localhost:8545".to_string(),
                "--chain-id".to_string(),
                "1".to_string(),
            ]
        );
    }
}
