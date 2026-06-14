# Core Concepts

`mevlog` works in three steps: **index → store → query**. Everything the tool
does is built on this single pipeline, so it is worth understanding before
diving into individual commands.

## Index

- mevlog fetches blocks from any EVM-compatible chain over RPC.
- With zero config it auto-selects the fastest RPC endpoint for the target
  chain from [Chainlist](https://chainlist.org/); you can also pin your own
  endpoints (see [RPC URLs](./rpc-urls.md)).
- Indexing pulls transactions, blocks, and logs for a block range. The `index`
  command backfills a range; `--live` keeps watching for new blocks.
- Every command that needs data first makes sure the relevant block(s) are
  indexed, so you rarely call `index` directly.

## Store

- Indexed data lands in a local **per-chain SQLite database**
  (`~/.mevlog/mevlog-txs-v1-{chain_id}.db`).
- The store has three tables: `transactions`, `blocks`, and `logs` (see
  [Database Schema](./schema.md)).
- Once a block is indexed it is cached locally, so repeat queries against the
  same range are almost instant and hit no RPC.
- A separate signatures database (`mevlog-sqlite-v5.db`) holds method/event
  signatures and chain metadata; it is downloaded prebuilt from a CDN on first
  run.

## Query

- Once data is local, everything else is a **read against SQLite**.
- The `query` command runs arbitrary read-only SQL against the store via
  `--sql`.
- Display commands (`tx`, `tx-logs`, `block`, `block-txs`, `block-logs`) are
  convenience wrappers: they index the needed block(s), then render the result
  with predefined SQL.
- U256 values are stored as big-endian BLOBs, so use the
  [SQL functions](./evm-sqlite-helpers.md) (`u256_sum`, `u256_mul`,
  `format_ether`, …) instead of plain SQL arithmetic on those columns.
- [SQL macros](./macros.md) like `{LATEST_BLOCK()}` and
  `{NATIVE_TOKEN_PRICE()}` expand to live values before the query runs.
