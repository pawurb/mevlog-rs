## mevlog-rs - explore Ethereum from your terminal [![Latest Version](https://img.shields.io/crates/v/mevlog.svg)](https://crates.io/crates/pg-extras) [![GH Actions](https://github.com/pawurb/mevlog-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/pawurb/mevlog-rs/actions)
 
![Big bribe](big-bribe-tx.png)

Rust-based CLI tool for querying and monitoring EVM blockchain transactions, with flexible filtering and EVM tracing capabilities. It's a tool for MEV searchers who prefer command-line workflows over web-based explorers.

`mevlog` allows you to analyze transaction details, detect validator bribes, track storage changes, search by block ranges, ENS domains, emitted event names, gas prices, and method calls. All while working on public RPC endpoints thanks to leveraging EVM tracing via [Revm](https://github.com/bluealloy/revm).

## Getting started

```bash
cargo install mevlog
mevlog watch --rpc-url https://eth.merkle.io --trace revm
```

On initial run `mevlog` downloads ~130mb [openchain.xyz signatures database](https://openchain.xyz/signatures) and extracts it to `~/.mevlog`. Signatures data allows displaying human readable info instead of hex blobs.

To avoid throttling on public endpoints `watch` mode displays only the top 5 transactions from each block.

You can change it using the `--position` argument:

```bash
## display the top 20 txs from each new block
mevlog watch -p 0:19 
```

## Supported filtering options

A few examples of currently supported queries:

- find `jaredfromsubway.eth` transactions from the last 20 blocks that landed in positions 0-5:

```bash
mevlog search -b 10:latest -p 0:5 --from jaredfromsubway.eth
```

- unknown method signature contract call in top position (likely an MEV bot):

```bash
mevlog search -b 10:latest --method "<Unknown>" -p 0
```

- query the last 50 blocks for transaction in the top 10 slots that transferred [PEPE](https://etherscan.io/token/0x6982508145454ce325ddbe47a25d4ec3d2311933) token:

```bash
mevlog search -b 50:latest -p 0:10 --event "Transfer(address,address,uint256)|0x6982508145454ce325ddbe47a25d4ec3d2311933"
```

- blocks between 22034300 and 22034320, position 0 transaction that did not emit any `Swap` events:

```bash
mevlog search -b 22034300:22034320 -p 0 --not-event "/(Swap).+/"
```

- blocks range for events containing `rebase` and `Transfer` keywords:

```bash
mevlog search -b 22045400:22045420 --event "/(?i)(rebase).+/" --event "/(Transfer).+/"
```

### EVM tracing filters

All the above queries use only standard block and logs input. By enabling `--trace [rpc|revm]` flag you can query by more conditions:

- query last 5 blocks for a top transaction that paid over 0.02 ETH total (including coinbase bribe) transaction cost:

```bash
mevlog search -b 5:latest -p 0 --real-tx-cost gte20000000000000000 --trace revm
```

You can also filter by real (including bribe) per gas price:

```bash
mevlog search -b 5:latest -p 0:5 --real-gas-price ge10000000000 --trace rpc
```

All the filter conditions can be combined. Here's a full list of currently supported filters:

```
Options:
  -f, --from <FROM>
          Filter by tx source address or ENS name
  -t, --touching <TOUCHING>
          Filter by contracts with storage changed by the transaction
      --event <EVENT>
          Include txs by event names matching the provided regex or string and optionally an address
      --not-event <NOT_EVENT>
          Exclude txs by event names matching the provided regex or string and optionally an address
      --method <METHOD>
          Include txs by root method names matching the provided regex or string
      --tx-cost <TX_COST>
          Filter by tx cost in wei (e.g., 'ge10000000000000000', 'le10000000000000000')
      --real-tx-cost <REAL_TX_COST>
          Filter by real (including coinbase bribe) tx cost in wei (e.g., 'ge10000000000000000', 'le10000000000000000')
      --gas-price <GAS_PRICE>
          Filter by effective gas price in wei (e.g., 'gte2000000000', 'lte2000000000')
      --real-gas-price <REAL_GAS_PRICE>
          Filter by real (including coinbase bribe) effective gas price in wei (e.g., 'gte2000000000', 'lte2000000000')
```

Both `search` and `watch` support the same filtering options.

## EVM tracing modes

### `--trace rpc` 

This mode uses the `debug_traceTransaction` method. It's usually not available on public endpoints.

### `--trace revm` 

This mode leverages Revm tracing by downloading all the relevant storage slots and running simulations locally. If you want to trace a transaction at position 10, Revm must first simulate all the previous transactions from this block. It can be slow and cause throttling from public endpoints. This mode works only on HTTP endpoints. Websockets are currently not supported.

Subsequent `revm` simulations for the same block and transaction range use cached data and should be significantly faster.

## Analyzing a single transaction data

```bash
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 --trace revm
```

This command displays info for a single target transaction. By adding `--before` `--after` arguments you can include surrounding transactions:

```bash
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 --trace revm -B 1 -A 1
```

You can reverse the display order by adding the `--reverse` flag.

## Project status

WIP, feedback appreciated.
