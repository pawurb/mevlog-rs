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

    #[command(flatten)]
    pub conn_opts: ConnOpts,
}

impl McpArgs {
    pub async fn run(&self) -> Result<()> {
        debug!(port = self.port, chain_id = ?self.conn_opts.chain_id, "resolving MCP server connection");
        let resolved = resolve_conn(&self.conn_opts).await?;
        info!(
            port = self.port,
            chain_id = resolved.chain_id,
            "starting MCP server"
        );
        mevlog::mcp_server::run_mcp_server(resolved.rpc_url, resolved.chain_id, self.port).await
    }
}
