## mevlog-rs - explore EVM chains from your terminal 
[![Latest Version](https://img.shields.io/crates/v/mevlog.svg)](https://crates.io/crates/mevlog) [![GH Actions](https://github.com/pawurb/mevlog-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/pawurb/mevlog-rs/actions)
 
![Big bribe](big-bribe-tx2.png)

Rust-based CLI tool for querying and monitoring Ethereum (or [any EVM-compatible chain](#supported-evm-chains)) transactions, with flexible filtering and EVM tracing capabilities. It's a tool for MEV searchers who prefer command-line workflows over web-based explorers.

There's also [a beta web version](https://mevlog.rs/) available.

`mevlog` allows you to analyze transaction details via a simple CLI interface. It currently offers the following features:

- regexp search by emitted event names  
- search by ENS domain names
- filter txs based on their position in a block
- search by root and internal method calls
- track smart contract storage changes
- detect validator bribes
- filter txs by value and real (including bribe) gas prices and cost
– filter by the amount of a specific ERC20 token sent

All while working on public RPC endpoints thanks to leveraging EVM tracing via [Revm](https://github.com/bluealloy/revm).

You can [check out this article](https://pawelurbanek.com/long-tail-mev-revm) for technical details on how this project is implemented.

## Getting started

Mevlog uses [cryo CLI](https://github.com/paradigmxyz/cryo) for fetching data. Please install it first by running:

```bash
cargo install cryo_cli
```

and then:

```bash
git clone https://github.com/pawurb/mevlog-rs
cd mevlog-rs
cargo install --path .
```

or install from the [crates.io](https://crates.io/crates/mevlog):

```bash
cargo install mevlog
mevlog watch --rpc-url https://eth.merkle.io 
```

On initial run `mevlog` downloads ~80mb [openchain.xyz signatures](https://openchain.xyz/signatures), and [chains data](https://github.com/ethereum-lists/chains) database to `~/.mevlog`. Signatures data allows displaying human readable info instead of hex blobs.

To avoid throttling on public endpoints `watch` mode displays only the top 5 transactions from each block.

You can change it using the `--position` argument:

```bash
## display the top 20 txs from each new block
mevlog watch -p 0:19 
```

## Filtering options

A few examples of currently supported queries:

- find `jaredfromsubway.eth` transactions from the last 20 blocks that landed in positions 0-5:

```bash
mevlog search -b 10:latest -p 0:5 --from jaredfromsubway.eth
```

- unknown method signature contract call in a top position (likely an MEV bot):

```bash
mevlog search -b 10:latest --method "<Unknown>" -p 0
```

- query the last 50 blocks for transaction in the top 20 slots that transferred [PEPE](https://etherscan.io/token/0x6982508145454ce325ddbe47a25d4ec3d2311933) token:

```bash
mevlog search -b 50:latest -p 0:20 --event "Transfer(address,address,uint256)|0x6982508145454ce325ddbe47a25d4ec3d2311933"
```

- blocks between 22034300 and 22034320, position 0 transaction that did not emit any `Swap` events:

```bash
mevlog search -b 22034300:22034320 -p 0 --not-event "/(Swap).+/"
```

- blocks range for events containing `rebase` and `Transfer` keywords:

```bash
mevlog search -b 22045400:22045420 --event "/(?i)(rebase).+/" --event "/(Transfer).+/"
```

- query by transactions that created a new smart contract:

```bash
mevlog search -b 22045400:22045420 --to CREATE
```

- find transactions that transferred more than 1 ETH:

```bash
mevlog search -b 10:latest --value ge1ether
```

- find transactions that transferred over 1 million USDC

```bash
mevlog search -b 10:latest --erc20-transfer "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48|ge1000gwei"
```

- find transactions that emitted any Transfer events for USDC and display amounts:

```bash
mevlog search -b 10:latest --erc20-transfer "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" --erc20-transfer-amount
```

### Event filters

The `--event` and `--not-event` options allow filtering transactions based on emitted events. The filter criteria can be:

- a contract address matching on any emitted events `0x6982508145454ce325ddbe47a25d4ec3d2311933`
- a full event signature `Transfer(address,uint256)`
- a regular expression pattern `/(?i)(rebase).+/`
- a combination of an event signature and a contract address `Transfer(address,uint256)|0x6982508145454ce325ddbe47a25d4ec3d2311933`

You can supply mutiple `--event` and `--not-event` flags for precise control over which transactions are included or excluded.

### Transfer filters

The `--erc20-transfer` option allows filtering transactions that emitted ERC20 Transfer events. The filter criteria can be:

- a contract address matching any transfer amount: `0xa0b86a33e6ba3bc6c2c5ed1b4b29b5473fd5d2de`
- a contract address with amount filtering: `0xa0b86a33e6ba3bc6c2c5ed1b4b29b5473fd5d2de|ge1000` (transfers >= 1000 tokens)
- amount operators: `ge` (greater or equal), `le` (less or equal)
- amount units: raw numbers, `ether`, `gwei`, etc.

You can supply multiple `--erc20-transfer` flags to match transfers from different tokens or with different amount criteria.

By default, transfer amounts are not displayed in the logs. Use the `--erc20-transfer-amount` flag to show transfer amounts alongside the Transfer events.

### EVM tracing filters

All the above queries use only standard block and logs input. By enabling `--trace [rpc|revm]` flag you can query by more conditions:

- query last 5 blocks for a top transaction that paid over 0.02 ETH total (including coinbase bribe) transaction cost:

```bash
mevlog search -b 5:latest -p 0 --real-tx-cost ge0.02ether --trace revm
```

- find txs that changed storage slots of the [Balancer vault contract](https://etherscan.io/address/0xba12222222228d8ba445958a75a0704d566bf2c8):

`mevlog search -b 10:latest --touching 0xba12222222228d8ba445958a75a0704d566bf2c8 --trace rpc`

You can also filter by real (including bribe) gas price:

```bash
mevlog search -b 5:latest -p 0:5 --real-gas-price ge10gwei --trace rpc
```

It's possible to search txs by their sub method calls:

```bash
mevlog search -b 5:latest -p 0:5 --calls "/(swap).+/" --trace rpc
```

All the filter conditions can be combined. Here's a complete list of currently supported filters:

```
Options:
  -f, --from <FROM>
          Filter by tx source address or ENS name
      --to <TO>
          Filter by tx target address or ENS name, or CREATE transactions
  -t, --touching <TOUCHING>
          Filter by contracts with storage changed by the transaction
      --event <EVENT>
          Include txs by event names matching the provided regex or signature and optionally an address
      --not-event <NOT_EVENT>
          Exclude txs by event names matching the provided regex or signature and optionally an address
      --method <METHOD>
          Include txs with root method names matching the provided regex, signature or signature hash
      --calls <CALLS>
          Include txs by subcalls method names matching the provided regex, signature or signature hash
      --show-calls
          Show detailed tx calls info
      --tx-cost <TX_COST>
          Filter by tx cost (e.g., 'le0.001ether', 'ge0.01ether')
      --real-tx-cost <REAL_TX_COST>
          Filter by real (including coinbase bribe) tx cost (e.g., 'le0.001ether', 'ge0.01ether')
      --gas-price <GAS_PRICE>
          Filter by effective gas price (e.g., 'ge2gwei', 'le1gwei')
      --real-gas-price <REAL_GAS_PRICE>
          Filter by real (including coinbase bribe) effective gas price (e.g., 'ge3gwei', 'le2gwei')
      --value <VALUE>
          Filter by transaction value (e.g., 'ge1ether', 'le0.1ether')
      --erc20-transfer <TRANSFER>
          Filter by Transfer events with specific address and optionally amount (e.g., '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48' or '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48|ge1000gwei')
      --erc20-transfer-amount
          Display transfer amounts in ERC20 Transfer event logs
      --failed 
          Show only txs which failed to execute
```

Both `search` and `watch` support the same filtering options.

## EVM tracing modes

### `--trace rpc` 

This mode uses the `debug_traceTransaction` method. It's usually not available on public endpoints.

### `--trace revm` 

This mode leverages Revm tracing by downloading all the relevant storage slots and running simulations locally. If you want to trace a transaction at position 10, Revm must first simulate all the previous transactions from this block. It can be slow and cause throttling from public endpoints. 

Subsequent `revm` simulations for the same block and transaction range use cached data and should be significantly faster.

## Analyzing a single transaction data

```bash
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 
```

This command displays info for a single target transaction. By adding `--before` `--after` arguments you can include surrounding transactions:

```bash
mevlog tx 0x06fed3f7dc71194fe3c2fd379ef1e8aaa850354454ea9dd526364a4e24853660 -b 1 -a 1
```

You can reverse the display order by adding the `--reverse` flag.

## Supported EVM chains

The project currently supports over 2k EVM chains by reading the metadata from [ethereum-list/chains](https://github.com/ethereum-lists/chains). But only a few chains display $USD txs prices from integrated [ChainLink oracles](https://docs.chain.link/data-feeds/price-feeds/addresses). I'm planning to work on improving the coverage.

If you use it with an unsupported chain, explorer URL and currency symbol is not displayed.

## Development

`tokio-console` feature adds support for [tokio-console](https://github.com/tokio-rs/console):

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo run --features=tokio-console --bin mevlog watch
```

`seed-db` feature enables action to populate signatures and chains metadata SQLite database:

```bash
cargo run --features=seed-db --bin mevlog seed-db
```

## Project status

WIP, feedback appreciated. I'm currently seeking a sponsor to help cover archive node costs for [mevlog.rs](https://mevlog.rs/). My goal is to make a hosted search web UI publicly available.
