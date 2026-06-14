# Database Schema

The per-chain transactions store (`mevlog-txs-v1-{chain_id}.db`) has three tables you can query with `query --sql`. 

Column hints below are not part of the type, but tell you how to write working queries:

| Hint | Meaning |
| --- | --- |
| `u256` | 32-byte big-endian BLOB; use `u256_sum` / `u256_mul` / `u256_add` / `u256_to_dec` |
| `addr` | 20-byte address BLOB; predicates need `X'..'` literals |
| `hash` | 32-byte hash BLOB |
| `selector` | 4-byte method selector BLOB |
| `unix` | unix epoch seconds |
| `0/1` | SQLite has no boolean; stored as `0` / `1` |

A `?` after a column name means it is nullable; all other columns are `NOT NULL`. Addresses and hashes in predicates must be written as blob literals (`X'a0b8...'`).

## `transactions`

| Column | Type | Hint |
| --- | --- | --- |
| `block_number` | BIGINT | |
| `tx_index` | BIGINT | |
| `tx_hash` | BLOB | hash |
| `nonce` | BIGINT | |
| `from_address` | BLOB | addr |
| `to_address?` | BLOB | addr |
| `value` | BLOB | u256 |
| `gas_limit` | BIGINT | |
| `gas_used` | BIGINT | |
| `effective_gas_price` | BIGINT | |
| `gas_price` | BIGINT | |
| `max_fee_per_gas` | BIGINT | |
| `max_priority_fee_per_gas` | BIGINT | |
| `transaction_type?` | BIGINT | |
| `success` | BOOLEAN | 0/1 |
| `coinbase_transfer?` | BLOB | u256 |
| `signature_hash?` | BLOB | selector |
| `signature?` | TEXT | |

## `blocks`

A row exists for every indexed block (even empty ones), so this table doubles as the indexed-block tracker.

| Column | Type | Hint |
| --- | --- | --- |
| `block_number` | INTEGER | |
| `block_hash` | BLOB | hash |
| `miner` | BLOB | addr |
| `gas_used` | BIGINT | |
| `timestamp` | BIGINT | unix |
| `base_fee_per_gas?` | BIGINT | |

## `logs`

| Column | Type | Hint |
| --- | --- | --- |
| `block_number` | BIGINT | |
| `tx_index` | BIGINT | |
| `log_index` | BIGINT | |
| `address` | BLOB | addr |
| `topic0?` | BLOB | hash |
| `topic1?` | BLOB | hash |
| `topic2?` | BLOB | hash |
| `topic3?` | BLOB | hash |
| `data` | BLOB | |
| `erc20_amount?` | BLOB | u256 |
| `signature?` | TEXT | |

## Signatures DB

The separate `mevlog-sqlite-v5.db` holds method/event signatures and chain metadata. It is downloaded prebuilt from a CDN and is not queried via `--sql`.
