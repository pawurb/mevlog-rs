# Introduction

**mevlog** is a simple way to query data from *any* EVM-compatible chain —
locally, on your own machine, without depending on third parties.

It is a command-line alternative to commercial blockchain indexers and data
APIs. Instead of paying for a hosted service and querying someone else's
database, `mevlog` downloads raw chain data straight from an RPC endpoint, stores
it in a local SQLite database, and lets you query it with plain SQL. **You own
the data.** No API keys, no rate limits, no vendor lock-in — just a file on your
disk that you can query as much as you want, offline.

## What you can do with it

- **Download blockchain data efficiently** — pull any block range from an RPC
  node into a local per-chain SQLite store.
- **Keep data in sync** — follow the chain head live and prune old blocks to
  cap storage.
- **Query with SQL** — run read-only SQL against indexed `transactions`,
  `blocks`, and `logs` tables, with custom SQLite helpers for working with
  256-bit integers, token amounts, ETH/gwei/USD formatting, and ENS names.
- **Define custom tables** — decode the logs you care about (swaps, transfers,
  any event) into typed columns via config.
- **Inspect single transactions and blocks** — render a tx, its logs, or a whole
  block, plus deeper EVM analysis (state diffs, call traces, coinbase payments).
- **Generate EVM tracing insights** — replay any transaction to extract storage
  state diffs, decoded call traces, and direct coinbase payments. Tracing runs
  either through a node's standard `debug_traceTransaction` RPC, or fully locally
  via [Revm](https://github.com/bluealloy/revm) — no tracing-enabled RPC required.
- **Talk to the blockchain over MCP** — expose mevlog as a Model Context
  Protocol server so an AI assistant can query chain data on your behalf.

## How it works

mevlog follows a three-step model: **index → store → query**.

1. **Index** — fetch a block range from an RPC endpoint (using
   [cryo](https://github.com/paradigmxyz/cryo) under the hood).
2. **Store** — write the transactions, blocks, and logs into a local,
   per-chain SQLite database under `~/.mevlog/`.
3. **Query** — run SQL against that local store. Because the data lives on your
   disk, queries are fast and free.

These docs walk through each step. See [Quick Start](./getting-started/quick-start.md)
to get set up and run your first query.
