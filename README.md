# mevlog-rs - query any EVM chain with SQL

[![Latest Version](https://img.shields.io/crates/v/mevlog.svg)](https://crates.io/crates/mevlog) [![Downloads](https://img.shields.io/crates/d/mevlog.svg)](https://crates.io/crates/mevlog) [![GH Actions](https://github.com/pawurb/mevlog-rs/actions/workflows/rust.yml/badge.svg)](https://github.com/pawurb/mevlog-rs/actions)

**mevlog** is an open-source (MIT) alternative to commercial blockchain indexers and data APIs. Instead of using a hosted service and querying an external database, it downloads raw chain data straight from an RPC endpoint, stores it in a local SQLite database, and lets you query it with plain SQL. **You own the data** - no API keys, no rate limits, no vendor lock-in, just a file on your disk you can query as much as you want, offline.

It follows a three-step model: **index → store → query**, against any [ChainList](https://chainlist.org/) chain or your own node.

## Quick Start

mevlog is published on crates.io. Install it with Cargo:

```bash
cargo install mevlog --locked
```

_On the first run a signatures DB is downloaded and indexed (max ~1min)._

Fetch and display the transactions in the latest Ethereum mainnet block:

```bash
mevlog block-txs -b latest --chain-id=1
```

With ZERO config, `mevlog` detects the fastest RPC endpoint from [ChainList](https://chainlist.org/) and uses it to download data. All data is cached in a local SQLite database (`~/.mevlog/`), so subsequent queries against the same block ranges are almost instant.

Run any read-only SQL against the local database with the `query` command:

```bash
# Find the most expensive TX in the given block range
mevlog query \
  -b 25314888:25314988 \
  --chain-id=1 \
  --sql "
    SELECT
      tx_hash,
      format_ether(u256_mul(gas_used, effective_gas_price)) AS cost
    FROM transactions
    ORDER BY u256_mul(gas_used, effective_gas_price) DESC
    LIMIT 1
  "
```

Produces:

```json
"result": [
  {
    "tx_hash": "0x6bd55342c59905fe4c8a25f43737f60c54d43334cc54472d08f4d0069748ce9a",
    "cost": "0.044113 ETH"
  }
],
```

mevlog adds EVM-native SQLite helpers (256-bit math, ERC20 decoding, ETH/gwei/USD formatting, ENS resolution) that plain SQL cannot do.

## Documentation & live demo

- Full documentation: https://mevlog.rs/docs
- Live SQL demo (last week of Mainnet data): https://mevlog.rs/search
