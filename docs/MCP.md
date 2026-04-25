# mevlog MCP Server

mevlog exposes an [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that lets AI assistants (Claude, Cursor, etc.) query Ethereum and EVM-compatible chains programmatically. All tools return JSON.

## Installation

The MCP server is behind a feature flag. Install mevlog with:

```bash
cargo install mevlog --features mcp
```

## Starting the server

```bash
mevlog mcp [OPTIONS]
```

| Option | Default | Env var | Description |
|---|---|---|---|
| `--host` | `127.0.0.1` | `MEVLOG_MCP_HOST` | Host/IP to bind (e.g. `127.0.0.1`, `::1`, `0.0.0.0`, `[::]`) |
| `--port` | `6671` | `MEVLOG_MCP_PORT` | HTTP server port |
| `--rpc-url` | — | — | RPC endpoint URL |
| `--chain-id` | — | — | Chain ID (auto-selects RPC from ChainList if `--rpc-url` is omitted) |

The server listens at `http://<host>:<port>/mcp` using the Streamable HTTP transport.

### Authentication

Set the `MEVLOG_MCP_AUTH_TOKEN` environment variable to require a Bearer token on every request. When unset, authentication is disabled.

```bash
MEVLOG_MCP_AUTH_TOKEN=my-secret mevlog mcp --chain-id 1
```

Clients must send `Authorization: Bearer my-secret` in the request headers.

## Client configuration

### Claude Desktop / Claude Code

Add to your MCP config (`claude_desktop_config.json` or `.mcp.json`):

```json
{
  "mcpServers": {
    "mevlog": {
      "url": "http://localhost:6671/mcp"
    }
  }
}
```

If authentication is enabled, add the header:

```json
{
  "mcpServers": {
    "mevlog": {
      "url": "http://localhost:6671/mcp",
      "headers": {
        "Authorization": "Bearer my-secret"
      }
    }
  }
}
```

## Tools

### `get_transaction`

Get detailed information about a specific transaction.

Returns a JSON object with `transactions`, `duration`, `chain`, and `query` fields.

#### Parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `tx_hash` | string | **yes** | Transaction hash (0x-prefixed hex string) |
| `evm_trace` | string | no | Tracing mode: `revm` for local simulation or `rpc` for `debug_traceTransaction` |
| `before` | integer | no | Number of transactions before (newer, smaller indexes) to include |
| `after` | integer | no | Number of transactions after (older, larger indexes) to include |
| `reverse` | boolean | no | Reverse the order of transactions |
| `evm_calls` | boolean | no | Show detailed tx calls info (requires `evm_trace`) |
| `evm_ops` | boolean | no | Display EVM opcodes executed (requires `evm_trace`) |
| `evm_state_diff` | boolean | no | Display storage slot changes / state diff (requires `evm_trace`) |
| `erc20_transfer_amount` | boolean | no | Display amounts in ERC20 Transfer event logs |
| `ens` | boolean | no | Enable ENS name resolution for addresses |
| `erc20_symbols` | boolean | no | Enable ERC20 token symbol resolution |
| `logs` | boolean | no | Include event logs in output |
| `native_token_price` | float | no | Native token price in USD (e.g., `3500.0`). Adds USD values for costs and transfers |

#### Example

```json
{
  "tx_hash": "0xabc...def",
  "evm_trace": "revm",
  "evm_calls": true,
  "logs": true
}
```

---

### `search_transactions`

Search for transactions matching filter conditions within a block range.

Returns a JSON object with `transactions`, `duration`, `chain`, and `query` fields.

#### Parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `blocks` | string | **yes** | Block range: `latest`, `22030899`, `22030800:22030900`, or `50:latest` (last 50 blocks) |
| `from` | string | no | Filter by sender address (0x-prefixed) or ENS name |
| `to` | string | no | Filter by recipient address (0x-prefixed), ENS name, or `CREATE` for contract creations |
| `event` | string | no | Filter by event signature name or regex (e.g., `Transfer` or `/Swap/`) |
| `not_event` | string | no | Exclude txs by event names matching regex or signature |
| `method` | string | no | Filter by root method signature name or regex |
| `calls` | string | no | Filter by subcall method names matching regex, signature, or signature hash (requires `evm_trace`) |
| `touching` | string | no | Filter by contracts with storage changed (0x-prefixed address, requires `evm_trace`) |
| `limit` | integer | no | Maximum number of transactions to return |
| `evm_trace` | string | no | Tracing mode: `revm` or `rpc` |
| `failed` | boolean | no | If true, only return failed transactions |
| `sort` | string | no | Sort by: `gas-price`, `gas-used`, `tx-cost`, `full-tx-cost`, or `erc20Transfer\|<token_address>` |
| `sort_dir` | string | no | Sort direction: `asc` or `desc` (default: `desc`) |
| `position` | string | no | Tx position or range within a block (e.g., `0` for first, `0:10` for first 11) |
| `tx_cost` | string | no | Filter by tx cost (e.g., `le0.001ether`, `ge0.01ether`) |
| `real_tx_cost` | string | no | Filter by real tx cost including coinbase bribe (e.g., `le0.001ether`) |
| `gas_price` | string | no | Filter by effective gas price (e.g., `ge2gwei`, `le1gwei`) |
| `real_gas_price` | string | no | Filter by real gas price including coinbase bribe (e.g., `ge3gwei`) |
| `value` | string | no | Filter by transaction value (e.g., `ge1ether`, `le0.1ether`) |
| `erc20_transfer` | string | no | Filter by ERC20 Transfer events with address and optional amount (e.g., `0xa0b8...\|ge1000gwei`) |
| `evm_calls` | boolean | no | Show detailed tx calls info (requires `evm_trace`) |
| `evm_ops` | boolean | no | Display EVM opcodes executed (requires `evm_trace`) |
| `evm_state_diff` | boolean | no | Display storage slot changes / state diff (requires `evm_trace`) |
| `erc20_transfer_amount` | boolean | no | Display amounts in ERC20 Transfer event logs |
| `ens` | boolean | no | Enable ENS name resolution for addresses |
| `erc20_symbols` | boolean | no | Enable ERC20 token symbol resolution |
| `logs` | boolean | no | Include event logs in output |
| `native_token_price` | float | no | Native token price in USD. Adds USD values for costs and transfers |

#### Example

```json
{
  "blocks": "50:latest",
  "event": "Swap",
  "limit": 10,
  "sort": "gas-price",
  "sort_dir": "desc"
}
```

---

### `list_chains`

List available EVM chains from ChainList.

Returns a JSON array of chains with `chain_id`, `name`, and explorer URL.

#### Parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `filter` | string | no | Filter chains by name (case-insensitive substring match) |
| `limit` | integer | no | Maximum number of chains to return |
| `chain_ids` | string | no | Comma-separated chain IDs to filter by |

#### Example

```json
{
  "filter": "arbitrum",
  "limit": 5
}
```

---

### `chain_info`

Get detailed information about a specific EVM chain.

Returns JSON with `chain_id`, `name`, currency symbol, and explorer URL.

#### Parameters

| Name | Type | Required | Description |
|---|---|---|---|
| `chain_id` | integer | **yes** | Chain ID to get information for |
| `include_rpcs` | boolean | no | Include RPC endpoints sorted by response time |

#### Example

```json
{
  "chain_id": 1,
  "include_rpcs": true
}
```

---

### `check_debug_available`

Check if the configured RPC endpoint supports `debug_traceTransaction`.

Takes no parameters. Returns `true` or `false`.

Debug tracing is required for features like `--evm-trace rpc`, internal call tracing, and state diffs. When `debug_traceTransaction` is not available, use `evm_trace: "revm"` instead for local EVM simulation.
