use eyre::Result;
use mevlog::misc::shared_init::{ConnOpts, resolve_conn};
use tracing::{debug, info};

#[derive(Debug, clap::Parser)]
pub struct McpArgs {
    #[arg(
        long,
        default_value = "6671",
        env = "MEVLOG_MCP_PORT",
        help = "Port for the MCP HTTP server"
    )]
    pub port: u16,

    #[arg(
        long,
        default_value = "127.0.0.1",
        env = "MEVLOG_MCP_HOST",
        help = "Host/IP to bind the MCP HTTP server (e.g. 127.0.0.1, ::1, 0.0.0.0, [::])"
    )]
    pub host: String,

    #[arg(
        long,
        default_value = "30000",
        env = "MEVLOG_MCP_TIMEOUT_MS",
        help = "Per-request timeout in milliseconds for the underlying mevlog CLI execution"
    )]
    pub timeout_ms: u64,

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl McpArgs {
    pub(crate) async fn run(&self) -> Result<()> {
        debug!(host = %self.host, port = self.port, chain_id = ?self.conn_opts.chain_id, "resolving MCP server connection");
        let resolved = resolve_conn(&self.conn_opts).await?;
        info!(
            host = %self.host,
            port = self.port,
            chain_id = resolved.chain_id,
            "starting MCP server"
        );
        mevlog::mcp_server::run_mcp_server(
            resolved.rpc_url,
            resolved.chain_id,
            &self.host,
            self.port,
            std::time::Duration::from_millis(self.timeout_ms),
        )
        .await
    }
}
