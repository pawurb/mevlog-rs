# Quick Start

## Install

mevlog is published on [crates.io](https://crates.io/crates/mevlog). Install it
with Cargo:

```bash
cargo install mevlog
```

## Run your first command

Fetch and display the transactions in the latest Ethereum mainnet block:

```bash
mevlog block-txs latest --chain-id=1
```

`--chain-id=1` selects Ethereum mainnet and auto-picks a working public RPC
endpoint from [ChainList](https://chainlist.org). mevlog indexes the block into
a local SQLite database under `~/.mevlog/`, then renders its transactions.
