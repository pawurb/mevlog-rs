# Coinbase transfers

A *coinbase transfer* is direct ETH a transaction pays to the block's `coinbase` (the miner/validator that produced the block), on top of the regular gas fee. Searchers use it to bid for inclusion: instead of (or in addition to) raising the gas price, the bot sends ETH straight to the block beneficiary from inside the transaction.

This is one of the most important MEV metrics. The size of the coinbase transfer is effectively the price a bot is willing to pay to land its bundle - it usually reflects the value of the arbitrage, liquidation, or sandwich the transaction is capturing. A large coinbase payment is a strong signal that a transaction is competing for a profitable MEV opportunity.

## How `query` records it

By default `query` does **not** compute coinbase transfers - the data isn't available in a block's transactions or receipts, it has to be reconstructed from execution traces.

When you pass `--evm-trace` (mode `rpc` or `revm`), `query` traces every newly indexed transaction in the block range, finds direct ETH transfers to the block's coinbase, and stores the wei amount in the `coinbase_transfer` column of the `transactions` table:

```bash
mevlog query -b 100:latest --evm-trace rpc \
  --sql "SELECT tx_hash, format_ether(coinbase_transfer) AS bribe
         FROM transactions
         WHERE coinbase_transfer IS NOT NULL AND coinbase_transfer > 0
         ORDER BY coinbase_transfer DESC LIMIT 20"
```

`coinbase_transfer` is a 32-byte big-endian U256 BLOB holding wei, so query it with the U256 helpers (`format_ether`, `u256_to_dec`, `u256_sum`), not plain SQL arithmetic.

Semantics of the column:

- **`NULL`** - the transaction was never traced (indexed without `--evm-trace`, or the trace failed / the block beneficiary was unknown).
- **`0`** - traced, but the transaction made no direct payment to the coinbase.
- **`> 0`** - the wei amount paid directly to the block's coinbase.

Backfill is incremental: re-running `query` with `--evm-trace` over a range traces only the rows still `NULL`, so blocks indexed earlier without tracing get filled in without re-tracing everything.

## Cost and throttling

EVM tracing is **slow and expensive**. Each transaction needs a full re-execution - either a `debug_traceTransaction` RPC call (`--evm-trace rpc`) or a local Revm replay against a forked state DB (`--evm-trace revm`). Both are far heavier than fetching plain block data.

On non-premium RPC endpoints this matters:

- `debug_*` tracing methods are often disabled, rate-limited, or billed at a higher tier than standard calls.
- Tracing a wide block range fires one trace per transaction (hundreds per block on a busy chain), which will hit rate limits and get you throttled or temporarily banned on free/shared endpoints.

Use `--evm-trace` deliberately: trace narrow ranges, prefer a premium or self-hosted node with `debug` tracing enabled, and only enable it when you actually need the coinbase-transfer metric.
