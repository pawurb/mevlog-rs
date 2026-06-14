# MCP Server

`mevlog` ships a [Model Context Protocol](https://modelcontextprotocol.io/) server that exposes the local transactions store to MCP-capable clients (Claude Code, Claude Desktop, any MCP SDK). It is gated behind the `mcp` feature, so install it with:

```bash
cargo install mevlog --features=mcp --locked
```

and verify the subcommand is present:

```bash
mevlog mcp --help
```

The server speaks the **streamable HTTP** transport over a single `/mcp` endpoint. It is read-only: it never indexes or fetches new blocks. See [Indexing](./indexing.md) for info on how to populate data.

## Running the server

```bash
MEVLOG_MCP_AUTH_TOKEN=<token> \
  mevlog mcp \
  --rpc-url='https://eth-mainnet.g.alchemy.com/v2/<API_KEY>'
```

Defaults: `--host 127.0.0.1`, `--port 6671`. The endpoint is then `http://127.0.0.1:6671/mcp`.

| Option | Env var | Default | Purpose |
|--------|---------|---------|---------|
| `--host` | `MEVLOG_MCP_HOST` | `127.0.0.1` | Bind address. Keep it `127.0.0.1` and put a TLS proxy in front (see below). |
| `--port` | `MEVLOG_MCP_PORT` | `6671` | Bind port. |
| `--rpc-url` | `MEVLOG_MCP_RPC_URL` | - | RPC endpoint for the chain the store covers. Used to resolve `{LATEST_BLOCK()}`, `{NATIVE_TOKEN_PRICE()}` and `{RESOLVE_ENS()}` macros. |
| `--chain-id` | - | derived from RPC | Chain the store is scoped to. |
| - | `MEVLOG_MCP_AUTH_TOKEN` | unset | Bearer token. If unset/empty, **auth is disabled** - always set it for anything reachable beyond localhost. |

## Auth

When `MEVLOG_MCP_AUTH_TOKEN` is set, every request must carry:

```
Authorization: Bearer <token>
```

## Tools

The server exposes two read-only tools.

### `query`

Runs read-only SQL against the per-chain SQLite store and returns a JSON `QueryResponse` envelope (`result`, `duration`, `chain`, `query`, where `query.sql` echoes the fully-substituted SQL that produced `result`). It never writes, indexes, or fetches blocks.

Parameters:

| Param | Type | Required | Description |
|-------|------|----------|-------------|
| `sql` | string | yes | Read-only SQL over the local store. |
| `native_token_price` | number | no | Native token price in USD (e.g. `3500.0`). Feeds the `{NATIVE_TOKEN_PRICE()}` macro and `convert_usd(wei, price)`. |
| `max_rows` | integer | no | Maximum rows the query may return; errors when exceeded. |

The schema, the U256/display helper functions (`u256_sum`, `u256_mul`, `format_ether`, `convert_usd`, …) and the `{LATEST_BLOCK()}` / `{NATIVE_TOKEN_PRICE()}` / `{RESOLVE_ENS()}` macros are the same as the `query` CLI command - see [Schema](./schema.md) and [Functions & Macros](./evm-sqlite-helpers.md).

Example `sql` payloads:

```sql
-- Total USDC transferred in the last 100 indexed blocks
SELECT u256_sum(erc20_amount) AS total
FROM logs
WHERE block_number > {LATEST_BLOCK()} - 100
  AND address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'
  AND erc20_amount IS NOT NULL;

-- Failed transactions in a block 
SELECT tx_hash, signature
FROM transactions
WHERE block_number = 22030899 AND success = 0;
```

### `db_info`

Takes no parameters. Returns read-only stats for the local per-chain transactions database (indexed block range, row counts, file size) for the server's configured chain. Equivalent to the `mevlog db-info` CLI command.
