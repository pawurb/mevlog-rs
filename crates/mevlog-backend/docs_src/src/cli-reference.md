# CLI Reference

```text
mevlog: Index EVM transactions into a local SQLite DB and query them with SQL across 2k+ chains

https://github.com/pawurb/mevlog-rs

Usage: mevlog [OPTIONS] <COMMAND>

Commands:
  query                   Collect txs from a block range and run read-only SQL against the local txs DB
  index                   Index a block range into the local txs DB
  reindex                 Refetch missing blocks within the local txs DB's indexed range
  purge-db                Remove indexed data below a block window ending at the newest indexed block
  db-info                 Show local txs DB stats
  tx                      Show a single transaction
  tx-logs                 Show a transaction's logs
  block                   Show a single block's metadata
  block-txs               Show a block's transactions
  block-logs              Show all logs in a block
  update-sigs-db          Update the signatures database
  update-custom-tables    Rebuild config-defined custom tables from indexed logs (requires --chain-id or --rpc-url; one run per chain)
  chains                  List all available chains from ChainList
  chain-info              Show detailed chain information
  evm-coinbase-transfer   Compute a tx's direct ETH payment to its block's coinbase
  evm-affected-addresses  List addresses affected by a tx
  evm-state-diff          Show the storage state diff produced by a tx
  evm-traces              Extract a tx's decoded call traces
  debug-available         Check if RPC supports debug tracing
  ens-resolve             Resolve an ENS name to an address
  ens-lookup              Reverse-resolve an address to an ENS name
  mcp                     Start MCP server
  tui                     Run TUI
  help                    Print this message or the help of the given subcommand(s)
```

## Global options

Available on every command (`mcp` and `tui` require their feature flags):

```text
      --color <COLOR>                  [default: auto] [possible values: always, auto, never]
      --format <FORMAT>                Output format ('json', 'json-pretty', 'csv', 'table', 'html');
                                       'csv', 'table' and 'html' are query-only [default: json-pretty]
      --html-path <HTML_PATH>          Directory for --format html output (default: current directory)
      --html-filename <HTML_FILENAME>  Filename for --format html output (default: mevlog-<content-hash>.html)
      --ipfs                           Upload the rendered --format output to IPFS and print a CID +
                                       gateway URL (query commands only; configure the [ipfs] block in
                                       config.toml)
  -h, --help                           Print help
  -V, --version                        Print version (root command only)
```

The `html` format renders a self-contained, click-to-sort HTML page and writes
it to `<--html-path or cwd>/<--html-filename or mevlog-<content-hash>.html>`,
printing the file path. With `--ipfs`, the rendered `--format` output is uploaded
to IPFS (Pinata or a local Kubo node, selected by the `[ipfs]` block in
`config.toml`) and a CID + gateway URL is printed instead. Both are only
meaningful for the query commands (`query`, `tx`, `tx-logs`, `block`,
`block-txs`, `block-logs`); the other commands reject `csv`/`table`/`html` and
ignore `--ipfs`.

Most data commands also share these connection / fetch options (omitted from the
per-command listings below to keep them short):

```text
      --rpc-url <RPC_URL>                The URL of the HTTP provider
      --chain-id <CHAIN_ID>              Chain ID to automatically select RPC URL from ChainList
      --rpc-timeout-ms <MS>              Timeout for filtering RPC URLs [default: 1000]
      --block-timeout-ms <MS>            Timeout for block fetching [default: 10000]
      --skip-verify-chain-id             Skip verifying --chain-id with data from --rpc-url
      --txs-db-dir <DIR>                 Override the per-chain txs SQLite DB directory (mainly for tests)
      --cryo-requests-per-second <N>     Max RPC requests/s for cryo block fetching [default: 25]
      --cryo-max-concurrent-requests <N> Max concurrent RPC requests for cryo [default: 10]
      --cryo-max-retries <N>             Max retries for cryo RPC provider errors [default: 8]
      --cryo-initial-backoff <MS>        Initial retry backoff for cryo RPC errors [default: 1000]
```

## query (alias: q)

Collect txs from a block range and run read-only SQL against the local txs DB.

```text
Usage: mevlog query [OPTIONS] --sql <SQL>

Options:
  -b, --blocks <BLOCKS>...   Block number or range (e.g. '22030899', 'latest',
                             '22030800:22030900', '50:latest', '50:')
      --sql <SQL>            Read-only SQL to run against the local txs DB
                             (tables: transactions, logs, blocks). Blob columns
                             (addresses, hashes) are output as 0x-hex; predicates
                             must use blob literals, e.g. WHERE from_address = X'1111...'.
                             Macros (wrapped in braces): {LATEST_BLOCK()},
                             {NATIVE_TOKEN_PRICE()}, {RESOLVE_ENS("name.eth")}.
      --evm-trace <MODE>     EVM tracing mode ('revm' or 'rpc')
      --native-token-price <P>  Native token price in USD instead of the price oracle
      --latest-offset <N>    Get N-offset latest block
      --latest-block <N>     Latest block number used to expand {LATEST_BLOCK()}, avoiding the RPC call
      --max-range <N>        Maximum allowed block range size
      --max-rows <N>         Max rows the --sql query may return; errors when exceeded (default: unlimited)
      --batch-size <N>       Batch size for data fetching [default: 100]
      --skip-index           Query the local store as-is (no block range resolution or fetching)
      --timeout-ms <MS>      Abort query (RPC, indexing and SQL) after this many ms (default: no timeout)
```

Plus the shared connection / fetch options.

## index

Index a block range into the local txs DB.

```text
Usage: mevlog index [OPTIONS]

Options:
  -b, --blocks <BLOCKS>      Block number or range. Required unless --live is set
      --live                 Keep watching for new blocks and index them as they arrive
      --poll-interval-ms <MS>  Polling interval when --live is set [default: 3000]
      --keep <KEEP>          With --live: after each round, delete data older than this many
                             blocks behind the newest indexed block
      --latest-offset <N>    Get N-offset latest block
      --max-range <N>        Maximum allowed block range size
      --batch-size <N>       Batch size for data fetching [default: 100]
```

Plus the shared connection / fetch options.

## reindex

Refetch missing blocks within the local txs DB's indexed range.

```text
Usage: mevlog reindex [OPTIONS]

Options:
      --batch-size <N>       Batch size for data fetching [default: 100]
      --keep <KEEP>          Only reindex blocks within this distance of the newest indexed
                             block; older gaps are left alone. Defaults to the whole indexed
                             range. Mirror the purge --keep window so reindex does not backfill
                             blocks that purge would immediately drop
```

Plus the shared connection / fetch options.

## purge-db

Remove indexed data below a block window ending at the newest indexed block.

```text
Usage: mevlog purge-db [OPTIONS] --keep <KEEP> --chain-id <CHAIN_ID>

Options:
      --keep <KEEP>          Keep blocks within this distance of the newest indexed block;
                             data below that window is deleted (0 purges everything)
      --chain-id <CHAIN_ID>  Chain ID of the local transactions DB to purge
      --reclaim              Reclaim freed disk space with VACUUM after purging. Off by default:
                             freed pages are reused by subsequent inserts, and VACUUM needs an
                             exclusive whole-DB lock that can block concurrent readers/writers
      --txs-db-dir <DIR>     Override the per-chain txs SQLite DB directory (mainly for tests)
```

## db-info

Show local txs DB stats.

```text
Usage: mevlog db-info [OPTIONS] --chain-id <CHAIN_ID>

Options:
      --chain-id <CHAIN_ID>  Chain ID of the local transactions DB to inspect
      --txs-db-dir <DIR>     Override the per-chain txs SQLite DB directory (mainly for tests)
```

## tx

Show a single transaction.

```text
Usage: mevlog tx [OPTIONS] <TX_HASH>

Arguments:
  <TX_HASH>  Transaction hash to display

Options:
      --evm-trace <MODE>        EVM tracing mode ('revm' or 'rpc'); enables coinbase/full cost
      --native-token-price <P>  Native token price in USD (overrides the chain oracle)
```

Plus the shared connection / fetch options.

## tx-logs

Show a transaction's logs.

```text
Usage: mevlog tx-logs [OPTIONS] <TX_HASH>

Arguments:
  <TX_HASH>  Transaction hash whose logs to display
```

Plus the shared connection / fetch options.

## block

Show a single block's metadata.

```text
Usage: mevlog block [OPTIONS] --block <BLOCK>

Options:
  -b, --block <BLOCK>     Block number or 'latest'
      --latest-offset <N> Get N-offset latest block
```

Plus the shared connection / fetch options.

## block-txs

Show a block's transactions.

```text
Usage: mevlog block-txs [OPTIONS] --block <BLOCK>

Options:
  -b, --block <BLOCK>       Block number or 'latest'
      --latest-offset <N>   Get N-offset latest block
      --native-token-price <P>  Native token price in USD (overrides the chain oracle)
```

Plus the shared connection / fetch options.

## block-logs

Show all logs in a block.

```text
Usage: mevlog block-logs [OPTIONS] --block <BLOCK>

Options:
  -b, --block <BLOCK>     Block number or 'latest'
      --latest-offset <N> Get N-offset latest block
```

Plus the shared connection / fetch options.

## update-sigs-db

Update the signatures database.

```text
Usage: mevlog update-sigs-db [OPTIONS]
```

Takes only the global options.

## update-custom-tables

Rebuild config-defined custom tables from indexed logs (requires `--chain-id` or `--rpc-url`; one run per chain).

```text
Usage: mevlog update-custom-tables [OPTIONS]
```

Plus the shared connection options.

## chains

List all available chains from ChainList.

```text
Usage: mevlog chains [OPTIONS]

Options:
  -f, --filter <FILTER>  Filter chains by name (case-insensitive substring match)
  -l, --limit <LIMIT>    Limit the number of chains returned
      --chain-id <ID>    Filter by specific chain IDs (can be used multiple times)
```

## chain-info

Show detailed chain information.

```text
Usage: mevlog chain-info [OPTIONS]

Options:
      --skip-rpcs            Skip RPC URL benchmarking and only show chain information
      --chain-id <CHAIN_ID>  Chain ID to get information for
      --rpc-url <RPC_URL>    RPC URL to derive chain ID from
      --rpc-timeout-ms <MS>  RPC timeout in milliseconds [default: 1000]
      --rpcs-limit <N>       Number of RPC URLs to return [default: 5]
```

## evm-coinbase-transfer

Compute a tx's direct ETH payment to its block's coinbase.

```text
Usage: mevlog evm-coinbase-transfer [OPTIONS] <TX_HASH>

Arguments:
  <TX_HASH>  Transaction hash to compute the direct coinbase payment for

Options:
      --evm-trace <MODE>  EVM tracing mode ('revm' or 'rpc')
```

Plus the shared connection options.

## evm-affected-addresses

List addresses affected by a tx.

```text
Usage: mevlog evm-affected-addresses [OPTIONS] <TX_HASH>

Arguments:
  <TX_HASH>  Transaction hash to inspect for affected addresses

Options:
      --evm-trace <MODE>  EVM tracing mode ('revm' or 'rpc')
```

Plus the shared connection options.

## evm-state-diff

Show the storage state diff produced by a tx.

```text
Usage: mevlog evm-state-diff [OPTIONS] <TX_HASH>

Arguments:
  <TX_HASH>  Transaction hash to compute the storage state diff for

Options:
      --evm-trace <MODE>  EVM tracing mode ('revm' or 'rpc')
```

Plus the shared connection options.

## evm-traces

Extract a tx's decoded call traces.

```text
Usage: mevlog evm-traces [OPTIONS] <TX_HASH>

Arguments:
  <TX_HASH>  Transaction hash to extract call traces for

Options:
      --evm-trace <MODE>  EVM tracing mode ('revm' or 'rpc')
```

Plus the shared connection options.

## debug-available

Check if RPC supports debug tracing.

```text
Usage: mevlog debug-available [OPTIONS] --rpc-url <RPC_URL>

Options:
      --rpc-url <RPC_URL>  RPC URL to check for debug tracing support
      --timeout-ms <MS>    Timeout in milliseconds [default: 5000]
```

## ens-resolve

Resolve an ENS name to an address.

```text
Usage: mevlog ens-resolve [OPTIONS] <NAME>

Arguments:
  <NAME>  ENS name to resolve to an address (e.g. 'vitalik.eth')
```

Plus the shared connection options.

## ens-lookup

Reverse-resolve an address to an ENS name.

```text
Usage: mevlog ens-lookup [OPTIONS] <ADDRESS>

Arguments:
  <ADDRESS>  Address to reverse-resolve to an ENS name
```

Plus the shared connection options.

## mcp

Start MCP server (requires the `mcp` feature). See [MCP Server](./mcp.md).

```text
Usage: mevlog mcp [OPTIONS]

Options:
      --port <PORT>      Port for the MCP HTTP server [env: MEVLOG_MCP_PORT] [default: 6671]
      --host <HOST>      Host/IP to bind (e.g. 127.0.0.1, ::1, 0.0.0.0, [::])
                         [env: MEVLOG_MCP_HOST] [default: 127.0.0.1]
      --timeout-ms <MS>  Per-request work budget in ms (RPC, indexing and SQL); the CLI
                         subprocess is force-killed a few seconds later if it does not exit
                         [env: MEVLOG_MCP_TIMEOUT_MS] [default: 30000]
```

Plus the shared connection options.

## tui

Run TUI (requires the `tui` feature). See [TUI Dashboard](./tui.md).

```text
Usage: mevlog tui [OPTIONS]
```

Takes the shared connection options.
