# RPC & Revm modes

The EVM analysis commands (`evm-coinbase-transfer`, `evm-affected-addresses`, `evm-state-diff`, `evm-traces`) need an execution trace of the transaction. The `--evm-trace` flag selects how that trace is produced. Both modes yield the same data; they differ in what they ask of the RPC endpoint and in their performance characteristics.

## EVM tracing modes

### `--evm-trace rpc`

This mode uses the node's `debug_traceTransaction` method to obtain the trace directly from the RPC endpoint. It's fast and requires no local replay, but `debug`-namespace methods are usually not available on public endpoints - they're typically exposed only by archive nodes or paid providers. Check whether an endpoint supports it with `mevlog debug-available --rpc-url <URL>`.

### `--evm-trace revm`

This mode leverages [Revm](https://github.com/bluealloy/revm) tracing by downloading all the relevant storage slots and running simulations locally. Because the transaction's result depends on the state left by everything before it in the block, Revm must first replay all earlier transactions: to trace a transaction at position 10, it simulates positions 0 through 9 first. This means many RPC calls to pull the required state, so it can be slow and cause throttling from public endpoints.

The upside is that it needs only standard JSON-RPC methods (`eth_getStorageAt`, `eth_getCode`, etc.), so it works against endpoints that don't expose the `debug` namespace.

Subsequent `revm` simulations for the same block and transaction range use cached data (state is stored locally) and should be significantly faster.

## Which to use

- Prefer `rpc` when your endpoint supports `debug_traceTransaction` - it's the fastest path and avoids replaying the block.
- Use `revm` when you only have a standard public RPC, or when you want fully local, reproducible replay; accept a slower first run, then enjoy cached follow-ups.
