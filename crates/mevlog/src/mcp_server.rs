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
struct QueryParams {
    #[schemars(
        description = "Block number or range to index before querying: 'latest', a single block like '22030899', or a range like '22030800:22030900', '50:latest', '50:'. Omit only when skip_index is true."
    )]
    blocks: Option<String>,
    #[schemars(
        description = "Read-only SQL run against the local txs DB. Tables: transactions, logs, blocks. See the tool description for the full schema, U256 helper functions and {MACRO()} reference."
    )]
    sql: String,
    #[schemars(
        description = "Native token price in USD (e.g. 3500.0 for ETH); also feeds the {NATIVE_TOKEN_PRICE()} macro and convert_usd(wei, price)"
    )]
    native_token_price: Option<f64>,
    #[schemars(description = "Index the block that is N blocks behind the chain's latest block")]
    latest_offset: Option<u64>,
    #[schemars(
        description = "Latest block number used to expand the {LATEST_BLOCK()} macro without an extra RPC call"
    )]
    latest_block: Option<u64>,
    #[schemars(description = "Maximum allowed block range size")]
    max_range: Option<u64>,
    #[schemars(description = "Maximum number of rows the query may return (errors when exceeded)")]
    max_rows: Option<usize>,
    #[schemars(description = "Batch size for data fetching (default: 100)")]
    batch_size: Option<usize>,
    #[schemars(
        description = "Skip indexing and query the local store as-is (no block range resolution or fetching). When true, 'blocks' is ignored."
    )]
    skip_index: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TxParams {
    #[schemars(description = "Transaction hash (0x-prefixed hex string)")]
    tx_hash: String,
    #[schemars(
        description = "EVM tracing mode: 'revm' for local simulation or 'rpc' for debug_traceTransaction; enables coinbase/full cost"
    )]
    evm_trace: Option<String>,
    #[schemars(
        description = "Native token price in USD (overrides the chain oracle); adds USD value/cost columns"
    )]
    native_token_price: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TxHashParams {
    #[schemars(description = "Transaction hash (0x-prefixed hex string)")]
    tx_hash: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EvmTxParams {
    #[schemars(description = "Transaction hash (0x-prefixed hex string)")]
    tx_hash: String,
    #[schemars(
        description = "EVM tracing mode (required): 'revm' for local simulation or 'rpc' for debug_traceTransaction"
    )]
    evm_trace: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BlockParams {
    #[schemars(description = "Block number or 'latest'")]
    block: String,
    #[schemars(description = "Resolve the block that is N blocks behind the chain's latest block")]
    latest_offset: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct BlockTxsParams {
    #[schemars(description = "Block number or 'latest'")]
    block: String,
    #[schemars(description = "Resolve the block that is N blocks behind the chain's latest block")]
    latest_offset: Option<u64>,
    #[schemars(
        description = "Native token price in USD (overrides the chain oracle); adds USD columns"
    )]
    native_token_price: Option<f64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EnsResolveParams {
    #[schemars(description = "ENS name to resolve to an address (e.g. 'vitalik.eth')")]
    name: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EnsLookupParams {
    #[schemars(description = "Address to reverse-resolve to an ENS name (0x-prefixed hex)")]
    address: String,
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
struct IndexParams {
    #[schemars(
        description = "Block number or range to index: 'latest', a single block like '22030899', or a range like '22030800:22030900', '50:latest', '50:'."
    )]
    blocks: String,
    #[schemars(description = "Index the block that is N blocks behind the chain's latest block")]
    latest_offset: Option<u64>,
    #[schemars(description = "Maximum allowed block range size")]
    max_range: Option<u64>,
    #[schemars(description = "Batch size for data fetching (default: 100)")]
    batch_size: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ReindexParams {
    #[schemars(description = "Batch size for data fetching (default: 100)")]
    batch_size: Option<usize>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ChainInfoParams {
    #[schemars(description = "Chain ID to get information for")]
    chain_id: u64,
    #[schemars(description = "Include RPC endpoints sorted by response time")]
    include_rpcs: Option<bool>,
    #[schemars(
        description = "Number of RPC URLs to return when include_rpcs is true (default: 5)"
    )]
    rpcs_limit: Option<usize>,
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
        description = r#"Run a read-only SQL query against indexed Ethereum transactions within a block range.

This is the primary tool. It indexes the requested `blocks` into a local per-chain SQLite store, then runs `sql` over it and returns a JSON `QueryResponse` envelope (`result`, `duration`, `chain`, `query` — `query.sql` echoes the fully-substituted SQL that produced `result`).

Block range examples (the `blocks` param): 'latest', '22030899' (single block), '22030800:22030900' (range), '50:latest' (last 50 blocks), '50:' (50 blocks up to latest). Set `skip_index: true` to query already-indexed data without fetching.

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

EXAMPLES:
  • Total USDC transferred in the last 100 blocks:
      blocks="100:latest", sql="SELECT u256_sum(erc20_amount) AS total FROM logs WHERE address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48' AND erc20_amount IS NOT NULL"
  • Top 10 most expensive transactions in a block (order by the raw cost, NOT by the decimal string — u256_to_dec/format_* produce TEXT that SQLite sorts lexicographically, giving a wrong ranking; sort on a numeric or blob expression and convert only in the projection). Here gas_used*effective_gas_price fits in INTEGER for ordering:
      blocks="22030899", sql="SELECT tx_hash, u256_to_dec(u256_mul(gas_used, effective_gas_price)) AS cost_wei FROM transactions ORDER BY gas_used * effective_gas_price DESC LIMIT 10"
  • Transactions from an ENS name in the last 50 blocks:
      blocks="50:latest", sql="SELECT tx_hash, format_ether(value) AS eth FROM transactions WHERE from_address = {RESOLVE_ENS(\"vitalik.eth\")}"
  • Failed transactions in a block (decoded method signature is the `signature` column, not `method`):
      blocks="22030899", sql="SELECT tx_hash, signature FROM transactions WHERE success = 0""#
    )]
    async fn query(&self, params: Parameters<QueryParams>) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(blocks = ?p.blocks, skip_index = ?p.skip_index, "MCP query request");
        let skip_index = p.skip_index == Some(true);
        let mut args = vec!["query".to_string()];
        // `blocks` and `--skip-index` are mutually exclusive on the CLI
        // (QueryArgs.blocks declares conflicts_with = "skip_index"), so never
        // forward -b when skipping indexing — the param doc says blocks is ignored.
        if !skip_index && let Some(blocks) = p.blocks {
            args.push("-b".to_string());
            args.push(blocks);
        }
        args.push("--sql".to_string());
        args.push(p.sql);
        if skip_index {
            args.push("--skip-index".to_string());
        }
        if let Some(offset) = p.latest_offset {
            args.push("--latest-offset".to_string());
            args.push(offset.to_string());
        }
        if let Some(latest_block) = p.latest_block {
            args.push("--latest-block".to_string());
            args.push(latest_block.to_string());
        }
        if let Some(max_range) = p.max_range {
            args.push("--max-range".to_string());
            args.push(max_range.to_string());
        }
        if let Some(max_rows) = p.max_rows {
            args.push("--max-rows".to_string());
            args.push(max_rows.to_string());
        }
        if let Some(batch_size) = p.batch_size {
            args.push("--batch-size".to_string());
            args.push(batch_size.to_string());
        }
        if let Some(price) = p.native_token_price {
            args.push("--native-token-price".to_string());
            args.push(price.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = r#"Show a single Ethereum transaction.

Returns a `QueryResponse` envelope whose `result` holds one display-shaped transaction row (hash, block, from/to, value, gas cost, method, success). Use `evm_trace` ('revm' or 'rpc') to enable coinbase/full-cost analysis. Supply `native_token_price` to add USD value/cost columns. Use the `tx_logs` tool for this transaction's event logs."#)]
    async fn tx(&self, params: Parameters<TxParams>) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(tx_hash = %p.tx_hash, evm_trace = ?p.evm_trace, "MCP tx request");
        let mut args = vec!["tx".to_string(), p.tx_hash];
        if let Some(evm_trace) = p.evm_trace {
            args.push("--evm-trace".to_string());
            args.push(evm_trace);
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
        description = r#"Show the event logs emitted by a single Ethereum transaction.

Returns a `QueryResponse` envelope whose `result` holds the transaction's log rows (address, topic0..topic3, data, decoded erc20_amount)."#
    )]
    async fn tx_logs(&self, params: Parameters<TxHashParams>) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(tx_hash = %p.tx_hash, "MCP tx_logs request");
        let mut args = vec!["tx-logs".to_string(), p.tx_hash];
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = r#"Show a single block's metadata.

Accepts a block number or 'latest', or use `latest_offset` to resolve the block N behind latest. Returns a `QueryResponse` envelope whose `result` holds the block row (hash, miner, gas_used, base fee, timestamp, transaction count)."#)]
    async fn block(&self, params: Parameters<BlockParams>) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(block = %p.block, "MCP block request");
        let mut args = vec!["block".to_string(), p.block];
        if let Some(offset) = p.latest_offset {
            args.push("--latest-offset".to_string());
            args.push(offset.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = r#"Show all transactions in a block.

Accepts a block number or 'latest', or use `latest_offset` to resolve the block N behind latest. Supply `native_token_price` to add USD columns. Returns a `QueryResponse` envelope whose `result` holds the block's display-shaped transaction rows. Use the `block_logs` tool for the block's event logs."#)]
    async fn block_txs(
        &self,
        params: Parameters<BlockTxsParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(block = %p.block, "MCP block_txs request");
        let mut args = vec!["block-txs".to_string(), p.block];
        if let Some(offset) = p.latest_offset {
            args.push("--latest-offset".to_string());
            args.push(offset.to_string());
        }
        if let Some(price) = p.native_token_price {
            args.push("--native-token-price".to_string());
            args.push(price.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(description = r#"Show all event logs in a block.

Accepts a block number or 'latest', or use `latest_offset` to resolve the block N behind latest. Returns a `QueryResponse` envelope whose `result` holds every log row in the block (address, topic0..topic3, data, decoded erc20_amount)."#)]
    async fn block_logs(
        &self,
        params: Parameters<BlockParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(block = %p.block, "MCP block_logs request");
        let mut args = vec!["block-logs".to_string(), p.block];
        if let Some(offset) = p.latest_offset {
            args.push("--latest-offset".to_string());
            args.push(offset.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Show stats for the local per-chain transactions database (indexed block range, row counts, file size) for the server's configured chain."#
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

    #[tool(
        description = r#"Index a block range into the local per-chain txs DB without running a query.

Fetches and stores the requested `blocks` (transactions, logs, block metadata); only blocks absent from the DB are fetched, so re-indexing a covered range is a no-op. Use this to pre-populate the store, then query later with `skip_index: true`. Returns an `IndexResponse` JSON envelope (block range, cached/new block counts, duration). (Continuous --live indexing is not exposed over MCP.)

Block range examples: 'latest', '22030899' (single block), '22030800:22030900' (range), '50:latest' (last 50 blocks), '50:' (50 blocks up to latest)."#
    )]
    async fn index(&self, params: Parameters<IndexParams>) -> Result<CallToolResult, McpError> {
        let p = params.0;
        debug!(blocks = %p.blocks, "MCP index request");
        let mut args = vec!["index".to_string(), "-b".to_string(), p.blocks];
        if let Some(offset) = p.latest_offset {
            args.push("--latest-offset".to_string());
            args.push(offset.to_string());
        }
        if let Some(max_range) = p.max_range {
            args.push("--max-range".to_string());
            args.push(max_range.to_string());
        }
        if let Some(batch_size) = p.batch_size {
            args.push("--batch-size".to_string());
            args.push(batch_size.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Refetch missing blocks within the local txs DB's already-indexed range (backfill gaps).

Reads the stored block range from the local store and re-runs indexing over it; only blocks absent from the DB are fetched, so a contiguous range is a no-op (`new_blocks` = 0). Safe to run on a schedule. Errors if the DB has no indexed blocks yet. Returns an `IndexResponse` JSON envelope (block range, cached/new block counts, duration)."#
    )]
    async fn reindex(&self, params: Parameters<ReindexParams>) -> Result<CallToolResult, McpError> {
        debug!(batch_size = ?params.0.batch_size, "MCP reindex request");
        let mut args = vec!["reindex".to_string()];
        if let Some(batch_size) = params.0.batch_size {
            args.push("--batch-size".to_string());
            args.push(batch_size.to_string());
        }
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Compute a transaction's direct ETH payment to its block's coinbase (a common MEV signal). Use `evm_trace` ('revm' or 'rpc') to select the tracing backend."#
    )]
    async fn evm_coinbase_transfer(
        &self,
        params: Parameters<EvmTxParams>,
    ) -> Result<CallToolResult, McpError> {
        let output = self.run_evm_cmd("evm-coinbase-transfer", params.0).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"List the addresses affected (touched) by a transaction. Use `evm_trace` ('revm' or 'rpc') to select the tracing backend."#
    )]
    async fn evm_affected_addresses(
        &self,
        params: Parameters<EvmTxParams>,
    ) -> Result<CallToolResult, McpError> {
        let output = self.run_evm_cmd("evm-affected-addresses", params.0).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Show the storage state diff (changed storage slots) produced by a transaction. Use `evm_trace` ('revm' or 'rpc') to select the tracing backend."#
    )]
    async fn evm_state_diff(
        &self,
        params: Parameters<EvmTxParams>,
    ) -> Result<CallToolResult, McpError> {
        let output = self.run_evm_cmd("evm-state-diff", params.0).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Extract a transaction's decoded call traces (internal calls). Use `evm_trace` ('revm' or 'rpc') to select the tracing backend."#
    )]
    async fn evm_traces(
        &self,
        params: Parameters<EvmTxParams>,
    ) -> Result<CallToolResult, McpError> {
        let output = self.run_evm_cmd("evm-traces", params.0).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Resolve an ENS name (e.g. 'vitalik.eth') to an Ethereum address. Ethereum mainnet only."#
    )]
    async fn ens_resolve(
        &self,
        params: Parameters<EnsResolveParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(name = %params.0.name, "MCP ens_resolve request");
        let mut args = vec!["ens-resolve".to_string(), params.0.name];
        self.push_conn_args(&mut args);
        let output = self.run_mevlog_cmd(&args).await?;
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    #[tool(
        description = r#"Reverse-resolve an Ethereum address to its primary ENS name. Ethereum mainnet only."#
    )]
    async fn ens_lookup(
        &self,
        params: Parameters<EnsLookupParams>,
    ) -> Result<CallToolResult, McpError> {
        debug!(address = %params.0.address, "MCP ens_lookup request");
        let mut args = vec!["ens-lookup".to_string(), params.0.address];
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
        let mut args = vec![
            "chain-info".to_string(),
            "--chain-id".to_string(),
            params.0.chain_id.to_string(),
        ];
        if params.0.include_rpcs == Some(true) {
            if let Some(limit) = params.0.rpcs_limit {
                args.push("--rpcs-limit".to_string());
                args.push(limit.to_string());
            }
        } else {
            args.push("--skip-rpcs".to_string());
        }
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

    async fn run_evm_cmd(&self, subcommand: &str, params: EvmTxParams) -> Result<String, McpError> {
        debug!(subcommand, tx_hash = %params.tx_hash, evm_trace = %params.evm_trace, "MCP evm command request");
        let mut args = vec![
            subcommand.to_string(),
            params.tx_hash,
            "--evm-trace".to_string(),
            params.evm_trace,
        ];
        self.push_conn_args(&mut args);
        self.run_mevlog_cmd(&args).await
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
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("mevlog", env!("CARGO_PKG_VERSION")),
            )
            .with_instructions(
                "mevlog MCP server. Provides tools for Ethereum transaction analysis, querying, and chain information.",
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
) -> eyre::Result<()> {
    let cancellation_token = CancellationToken::new();
    let server_rpc_url = rpc_url.clone();

    let config = StreamableHttpServerConfig::default()
        .with_sse_keep_alive(Some(Duration::from_secs(15)))
        .with_sse_retry(None)
        .with_cancellation_token(cancellation_token.clone());

    let service = StreamableHttpService::new(
        move || Ok(MevlogMcpServer::new(server_rpc_url.clone(), chain_id)),
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
        let server = crate::mcp_server::MevlogMcpServer::new("http://localhost:8545".into(), 1);
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
