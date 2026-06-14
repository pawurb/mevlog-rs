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
mevlog block-txs -b latest --chain-id=1
```

_BTW on first execution of `mevlog` a signatures DB has to be downloaded and indexed, but it should take ~1min max_.

It should produce a similar JSON output:

```json
  // ...
      "display_gas_price": "0.16 gwei",
      "tx_cost": "3437206778184",
      "display_tx_cost": "0.000003 ETH",
      "display_tx_cost_usd": "$0.01"
    }
  ],
  "result_count": 156,
  "cached_blocks": 0,
  "new_blocks": 1,
  "duration": "1.49 s",
  "chain": {
    "chain_id": 1,
    "name": "Ethereum Mainnet",
    "currency": "ETH",
    "explorer_url": "https://etherscan.io",
    "native_token_price": 1674.61477
  },
  // ...
```

What just happened? You queried a Mainnet blockchain with ZERO config. Under the hood `mevlog` fetches the fastest RPC endpoint from [Chainlist](https://chainlist.org/).

The first execution against a target block might take a few seconds. But later ALL the data is cached in a local SQLite database (located in `~/.mevlog/`) so subsequent queries against the same block ranges are almost instant!

`mevlog` comes with a full blown chains explorer TUI interface. Install it by running:

```bash
cargo install mevlog --features=tui
```

and run:

```bash
mevlog tui
```


`--chain-id=1` selects Ethereum mainnet and auto-picks a working public RPC
endpoint from [ChainList](https://chainlist.org). mevlog indexes the block into
a local SQLite database under `~/.mevlog/`, then renders its transactions.
