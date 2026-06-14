# Functions & Macros

`mevlog` registers extra SQLite functions on the read-only `query` connection for working with the U256 BLOB columns and for display formatting, plus pre-query macros that expand to live values before the SQL runs. Plain SQL `SUM()` / `*` cannot handle 32-byte BLOBs or amounts that overflow a signed 64-bit `INTEGER`, so use these instead.

The functions come from the [`evm-sqlite-rs`](https://github.com/pawurb/evm-sqlite-rs) crate. Every U256 operand may be a non-negative `INTEGER` or a big-endian BLOB (≤ 32 bytes), and `NULL` generally propagates to `NULL`.

## Function reference

| Function | Returns | What it does |
| --- | --- | --- |
| `u256_sum(x)` | BLOB (`0x`-hex) | Aggregate. Sums U256 BLOB columns (`value`, `erc20_amount`, …) with exact arithmetic. Skips `NULL`, returns `NULL` over an empty set, raises on overflow. Plain `SUM()` cannot total these BLOBs. |
| `u256_add(a, b)` | BLOB | Exact 256-bit add; raises on overflow. `NULL` if either operand is `NULL`. |
| `u256_mul(a, b)` | BLOB | Exact 256-bit multiply; raises past `U256::MAX`. E.g. `u256_mul(gas_used, effective_gas_price)` for tx cost, which overflows a 64-bit `INTEGER`. |
| `u256_to_dec(x)` | TEXT | Decode a U256 BLOB to a full-precision decimal string (no precision loss). |
| `erc20_to_real(amount, decimals)` | REAL | Divide a token amount by `10^decimals` for direct numeric SQL. `decimals` is an `INTEGER` in `0..=77`. Approximate `f64` - use `u256_to_dec` for exact math. |
| `format_ether(x)` | TEXT | Render a wei amount as `"X.XXXXXX ETH"` (6 dp). |
| `format_gwei(x)` | TEXT | Render a wei amount as `"X.XX gwei"` (2 dp). |
| `convert_usd(wei, price)` | REAL | Convert a wei amount to its USD value, as `ether(wei) * price`. Approximate. `NULL` amount or price yields `NULL`. |
| `format_usd(x)` | TEXT | Pure formatter: render a REAL/INTEGER USD value as `"$X,XXX.XX"` (thousands commas, 2 dp). Does **not** convert from wei - wrap a wei amount in `convert_usd` first. |

The `convert_usd` / `format_usd` split is intentional: `convert_usd(wei, price)` does the wei→USD math, `format_usd(value)` only formats the resulting number. To render a wei column as a `$` string you compose them: `format_usd(convert_usd(t.value, {NATIVE_TOKEN_PRICE()}))`.

## Macros

Macros are expanded by the `query` command **before** the SQL runs, fetching each value over RPC only when its token is present. Rules:

- Every macro must be wrapped in braces and is strict, case-sensitive.
- The substituted literal is echoed back in the `query.sql` of the response, so the returned SQL fully describes what ran.

| Macro | Expands to |
| --- | --- |
| `{LATEST_BLOCK()}` | The chain's current latest block number. |
| `{NATIVE_TOKEN_PRICE()}` | The native token's USD price (from `--native-token-price` or the chain's Chainlink oracle). Errors if no price is available rather than emitting a wrong value. |
| `{RESOLVE_ENS("name.eth")}` | The resolved address as an `X'..'` blob literal. Ethereum mainnet only; the name must end in `.eth` and resolve, otherwise it errors. |

## Sample queries

### Total USDC transferred in the last 100 blocks

`u256_sum` totals the 32-byte `erc20_amount` BLOBs (plain `SUM()` would not work); `erc20_to_real(..., 6)` divides by `10^6` because USDC has 6 decimals. The address predicate is a blob literal.

```sql
SELECT erc20_to_real(u256_sum(erc20_amount), 6) AS total_usdc
FROM logs
WHERE address = X'a0b86991c6218b36c1d19d4a2e9eb0ce3606eb48'
  AND erc20_amount IS NOT NULL
  AND block_number >= {LATEST_BLOCK()} - 100
```

### Gas an ENS-named account spent in the last day

`{RESOLVE_ENS(...)}` becomes the account's address blob; `u256_mul(gas_used, effective_gas_price)` is the per-tx cost in wei (it overflows a 64-bit `INTEGER`, so `u256_mul` is required); `u256_sum` totals it; `format_ether` renders ETH and `convert_usd` + `format_usd` render the USD value.

```sql
SELECT COUNT(*) AS txs,
       format_ether(u256_sum(u256_mul(t.gas_used, t.effective_gas_price))) AS gas_spent_eth,
       format_usd(convert_usd(u256_sum(u256_mul(t.gas_used, t.effective_gas_price)), {NATIVE_TOKEN_PRICE()})) AS gas_spent_usd
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE t.from_address = {RESOLVE_ENS("jaredfromsubway.eth")}
  AND b.timestamp >= unixepoch('now', '-1 day')
```

### Top 10 ETH transfers in the last day

`value` is a U256 wei BLOB; `format_ether` renders it as ETH and `convert_usd` + `format_usd` price it in USD. Ordering is on the raw `value` BLOB, which sorts correctly because it is fixed-width big-endian.

```sql
SELECT t.tx_hash,
       format_ether(t.value) AS value_eth,
       format_usd(convert_usd(t.value, {NATIVE_TOKEN_PRICE()})) AS value_usd
FROM transactions t
JOIN blocks b ON b.block_number = t.block_number
WHERE b.timestamp >= unixepoch('now', '-1 day')
ORDER BY t.value DESC
LIMIT 10
```

### Exact decimal amount with no precision loss

`erc20_to_real` returns an approximate `f64`; when you need the exact integer value use `u256_to_dec`, which decodes the BLOB to a full-precision decimal string.

```sql
SELECT t.tx_hash, u256_to_dec(t.value) AS value_wei
FROM transactions t
ORDER BY t.value DESC
LIMIT 5
```
