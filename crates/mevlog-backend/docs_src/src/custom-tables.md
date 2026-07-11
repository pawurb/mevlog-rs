# Custom Tables

Define your own tables in `~/.mevlog/config.toml`, populated from indexed logs matching a `topic0`. Query them alongside the built-in tables.

## What they are

Custom tables are extra tables in the per-chain txs DB whose shape and contents you declare in config. They are pure derived data: every indexed block keeps its raw topics and `data` in the `logs` table, so a custom table is just an `INSERT INTO ... SELECT` over `logs`, with all decoding expressed in SQL. Nothing extra is fetched over RPC.

This lets you pull any columns you want out of event data that is otherwise not indexed. The `logs` table only breaks out `topic0..topic3`, `data`, and a decoded ERC20 transfer amount; everything else (swap amounts, tick values, any non-indexed ABI parameter) sits packed inside the raw `data` blob. A custom table slices those fields into their own typed, queryable columns.

Each table is tracked in a `custom_tables` meta table by `name` + a `fingerprint` (a hash of its `topic0`, addresses, and ordered columns). On every command that opens the txs DB, mevlog reconciles config against that meta table:

- missing table -> create it and backfill from all existing `logs`
- fingerprint matches -> no-op
- fingerprint changed, or a non-tracked table squats on the name -> error pointing you at `update-custom-tables`

After each indexing chunk lands, the applicable tables are populated for that block range, so they stay in step with `logs`. Row identity is `(block_number, log_index)` with `ON CONFLICT DO NOTHING`, so re-indexing is idempotent.

## Defining a table: indexing all swaps

This is the example shipped in the default `~/.mevlog/config.toml`. It captures Uniswap V2 `Swap` events from every pair, on every chain, decoding both indexed topics and the four packed `uint256` amounts out of the log `data`:

```toml
# Uniswap V2 Swap events from every pair (no emitter filter, all chains).
# Swap(address indexed sender, uint amount0In, uint amount1In,
#      uint amount0Out, uint amount1Out, address indexed to)
[tables.swaps]
topic0 = "0xd78ad95fa46c994b6551d0da85fc275fe613ce37657fb8d5e3d130840159d822"

[[tables.swaps.columns]]
name = "sender"
source = "topic1"
type = "address"

[[tables.swaps.columns]]
name = "to_address"
source = "topic2"
type = "address"

[[tables.swaps.columns]]
name = "amount0_in"
source = "data[0:32]"
type = "uint256"

[[tables.swaps.columns]]
name = "amount1_in"
source = "data[32:64]"
type = "uint256"

[[tables.swaps.columns]]
name = "amount0_out"
source = "data[64:96]"
type = "uint256"

[[tables.swaps.columns]]
name = "amount1_out"
source = "data[96:128]"
type = "uint256"
```

The `[tables.<name>]` header names the table (here `swaps`). It must match `^[a-z_][a-z0-9_]*$` and cannot be a reserved name (`transactions`, `blocks`, `logs`, `custom_tables`, `_sqlx_migrations`, or anything starting with `sqlite_`). Keys:

- `topic0` (required) - the 32-byte event signature hash. Only logs whose `topic0` equals this are captured. This is the only required selector and it is what makes the table event-specific.
- `chains` (optional) - list of chain IDs the table applies to, e.g. `chains = [1, 42161]`. Omit it (as above) to apply to every chain.
- `addresses` (optional) - emitter allowlist, e.g. `addresses = ["0x..."]`. Omit it to capture matching logs from any contract.
- `[[tables.<name>.columns]]` (at least one) - the decoded columns.

Every table defines four implicit columns: `block_number`, `tx_index`, `log_index`, `address` (the emitter). Your column names must not collide with those.

## Works for any event type

Nothing about this is swap-specific. Point `topic0` at any event signature and map its fields, and you get a typed table for that event. The same mechanism indexes ERC20 `Transfer`s, Uniswap V3 `Swap`s, `Sync` reserves, NFT `Transfer`s, governance votes, or any custom contract event - including non-indexed parameters that live only in `data` and are not otherwise queryable. Define one `[tables.<name>]` block per event you care about.

## Column mapping

Each column has a `name`, a `source`, and a `type`.

`source` selects the bytes:

- `topic1`, `topic2`, `topic3` - an indexed event parameter (`topic0` is the match key, not a source, so it is not selectable).
- `data[start:end]` - a byte range of the log `data`, 0-based and end-exclusive. ABI words are 32 bytes, so word `n` is `data[n*32:(n+1)*32]` (word 0 is `data[0:32]`, word 1 is `data[32:64]`, and so on).

`type` decides how the slice is decoded and stored (all columns are stored as `BLOB`):

- `address` - 20-byte blob. A 32-byte source (a topic, or a 32-byte data range) gets its 12-byte ABI left-pad stripped automatically. A data-range address source must be 20 or 32 bytes.
- `uint256` - 32-byte big-endian blob. A data range must be at most 32 bytes; shorter ranges are left-padded to 32. These work directly with the U256 SQL helpers (`u256_sum`, `u256_to_dec`, `format_ether`, etc.).
- `bytes` - the raw slice, stored verbatim. Requires a `data[start:end]` source.

Caveat: dynamic ABI parameters (`string`, `bytes`, arrays) are stored at the head of `data` as a 32-byte offset pointing elsewhere in the payload, not inline. A fixed `data[...]` range over a dynamic parameter captures that offset word, not the value. Custom tables are best suited to fixed-layout (static) parameters.

## Rebuilding after edits

Because contents are fingerprinted, editing a table's `topic0`, `addresses`, or columns changes its fingerprint, and the next run that opens the DB will refuse to continue rather than silently serve a stale shape. Rebuild it (lossless, offline, no RPC):

```bash
mevlog update-custom-tables --chain-id <id>
```

This drops every tracked custom table (including ones you removed from config) plus anything squatting on a configured name, then recreates and backfills the currently-configured tables from `logs`. Only the named chain's DB is touched, so a multi-chain config needs one run per chain.

## Querying custom tables

Configured custom table names are added to the read allowlist for `--sql`, alongside the built-in `transactions` / `blocks` / `logs`. Query them like any other table:

```bash
# Total USDC bought from the USDC/WETH Uniswap V2 pair over the last 1000 blocks.
# USDC is token0 (6 decimals) and a dollar stablecoin, so erc20_to_real on the
# amount0_out column is the USD value directly; format_usd renders it as a
# "$X,XXX.XX" string.
mevlog query -b 1000:latest --chain-id 1 \
  --sql "SELECT format_usd(erc20_to_real(u256_sum(amount0_out), 6)) AS usdc_bought
         FROM swaps
         WHERE address = X'b4e16d0168e52d35cacd2c6185b44281ec28c9dc'"
```

Sample response:

```json
{
  "result": [
    { "usdc_bought": "$41,873,204.55" }
  ],
  "result_count": 1,
  "cached_blocks": 1000,
  "new_blocks": 0,
  "duration": "182.44 ms",
  "generated_at": "2026-07-11T13:19:35Z",
  "chain": {
    "chain_id": 1,
    "name": "Ethereum Mainnet",
    "currency": "ETH",
    "explorer_url": "https://etherscan.io",
    "native_token_price": 3812.50
  },
  "query": {
    "blocks": "1000:latest",
    "sql": "SELECT format_usd(erc20_to_real(u256_sum(amount0_out), 6)) AS usdc_bought FROM swaps WHERE address = X'b4e16d0168e52d35cacd2c6185b44281ec28c9dc'"
  }
}
```
