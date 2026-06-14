# config.toml

`mevlog` reads optional settings from a TOML config file at `~/.mevlog/config.toml`. The file is created with a commented-out template on first run; running without it is fine, every option has a default.

Two top-level sections are supported: `[chains.<id>]` and `[tables.<name>]`.

## `[chains.<id>]` - custom RPC endpoints

By default `mevlog` auto-selects the fastest public RPC endpoint for a chain from [ChainList](https://chainlist.org/). To pin your own endpoint (e.g. a private Alchemy/Infura URL, or a chain ChainList does not cover), add a `[chains.<chain_id>]` section keyed by chain ID:

```toml
[chains.1]
rpc_url = "https://eth-mainnet.g.alchemy.com/v2/YOUR_API_KEY"

[chains.42161]
rpc_url = "https://arb-mainnet.g.alchemy.com/v2/YOUR_API_KEY"
```

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `rpc_url` | string | yes | HTTP(S) RPC endpoint used for that chain ID. Overrides ChainList auto-selection. |

The section key is the chain ID. Both `[chains.1]` and `[chains."1"]` are accepted.

## `[tables.<name>]` - custom tables

Define extra tables in the local txs database, populated from indexed `logs` rows matching a `topic0`, with topics and `data` byte ranges mapped to typed columns.

| Key | Type | Required | Description |
|-----|------|----------|-------------|
| `topic0` | hex string (32 bytes) | yes | Event signature hash the table matches. |
| `chains` | array of chain IDs | no | Restrict the table to these chains. Default: all chains. |
| `addresses` | array of hex addresses (20 bytes) | no | Emitter filter; only logs from these addresses are captured. Default: no filter. |
| `[[tables.<name>.columns]]` | array of tables | yes (≥1) | Column definitions (see below). |

Each `[[tables.<name>.columns]]` entry:

| Key | Type | Description |
|-----|------|-------------|
| `name` | string | Column name; must match `^[a-z_][a-z0-9_]*$` and not collide with implicit columns (`block_number`, `tx_index`, `log_index`, `address`). |
| `source` | string | `topic1`..`topic3`, or a 0-based end-exclusive data byte range like `data[0:32]` (ABI word *n* is `data[n*32:(n+1)*32]`). |
| `type` | string | `address` (20-byte BLOB), `uint256` (32-byte big-endian BLOB, works with `u256_*`/`format_ether`), or `bytes` (verbatim slice; requires a data range source). |

After editing a table's definition, rebuild it with `mevlog update-custom-tables --chain-id <id>`.

See [Custom Tables](./custom-tables.md) for a full walkthrough, query examples, and how the tables stay in step with `logs`.
