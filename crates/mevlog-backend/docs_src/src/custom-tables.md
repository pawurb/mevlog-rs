# Custom Tables

> Define your own tables in `~/.mevlog/config.toml`, populated from indexed
> logs matching a `topic0`. Query them alongside the built-in tables.

## What they are

- TODO: config-defined tables in the local txs DB, built from matching logs.
- TODO: tracked in the `custom_tables` meta table by name + fingerprint.

## Defining a table

- TODO: `[tables.<name>]` block, `topic0`, optional `chains`, optional
  `addresses` emitter filter.
- TODO: example TOML (Uniswap V2 `Swap`).

## Column mapping

- TODO: `source` = `topic1..topic3` or `data[start:end]` (0-based, end-exclusive;
  ABI word n is `data[n*32:(n+1)*32]`).
- TODO: types - `address` (20-byte BLOB), `uint256` (32-byte BE BLOB, works with
  `u256_sum` / `format_ether`), `bytes` (verbatim slice).
- TODO: caveat - dynamic ABI params (string/bytes) store the offset word, not the
  value.

## Rebuilding after edits

- TODO: `mevlog update-db --rebuild-tables --chain-id <id>`.
- TODO: fingerprint change triggers rebuild; name validation rejects reserved
  names.

## Querying custom tables

- TODO: custom table names are allowlisted for `--sql` reads alongside
  `transactions` / `blocks` / `logs`.
