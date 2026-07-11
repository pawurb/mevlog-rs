# Indexing

`mevlog` reads RPC endpoints to cache data in a local SQLite database. This doc section describes commands you can use to control the indexing process.

## Implicit indexing

`mevlog query` (and every display command: `tx`, `block`, `block-txs`, ...) expects a `--blocks` / `-b` parameter naming the block range to operate on. Before the SQL runs, mevlog makes sure every block in that range is present in the local store, fetching only what is missing.

**The `--blocks` parameter**

`--blocks` (alias `-b`) accepts four input formats:

| Format | Meaning |
| --- | --- |
| `latest` | The current latest block only. |
| `N` | A single block number `N`. |
| `N:M` | The inclusive range from block `N` to block `M`. |
| `N:latest` (or `N:`) | The last `N` blocks, ending at the latest block. |

Validation: in `N:M` the start must be `<=` the end, and neither a single block nor a range end may exceed the chain's current latest block.

**How missing blocks are detected**

1. The range is resolved to concrete block numbers (`latest` and `N:` are expanded via one RPC call for the current head).
2. mevlog reads the `block_number`s already present in the `blocks` table for that range. Because a row exists for every indexed block - including empty ones - the `blocks` table itself is the indexed-block tracker; any number in the range without a row is considered missing.
3. Only the missing blocks are fetched over RPC and indexed into the store. Blocks that are already cached are reused untouched, so repeat queries over the same range hit no RPC.
4. The JSON envelope reports the split as `cached_blocks` (already present) and `new_blocks` (fetched this run). Every output format also echoes the chain's latest block at query time (`latest_block` in JSON, a `latest_block:` line after table output, a `latest block` entry in the HTML meta line; CSV stays bare) for context on how fresh the queried data is.

**The `--skip-index` flag**

`mevlog query --skip-index` skips indexing entirely and runs the SQL against the local store as-is: no block range resolution and no RPC fetching. Use it to query already-cached data without touching the network. It is mutually exclusive with `--blocks` (pass one or the other, not both), and both `cached_blocks` and `new_blocks` are reported as `0` since nothing is resolved or fetched. `latest_block` is omitted too - the chain head is never resolved - unless an explicit `--latest-block` value is passed.

## `index` command

While `query` indexes on demand, `index` lets you populate the store ahead of time, and optionally keep it following the chain head.

```bash
# Backfill an explicit range
mevlog index -b 22030800:22030900 --chain-id 1

# Follow the chain head, keeping only the last 1000 blocks
mevlog index --live --keep 1000 --chain-id 1
```

- **`--blocks` / `-b`** - the range to backfill, using the same four formats as `query` (see above). Required unless `--live` is set. The same fetch-only-missing logic applies, so re-running over an already-indexed range is cheap.
- **`--live`** - after the initial backfill, keep polling for new blocks and index each new one as it arrives. With `--live` you may omit `--blocks`, in which case watching starts from the current latest block.
- **`--poll-interval-ms`** - how often to poll for a new head in live mode (default `3000`).
- **`--keep N`** - in live mode only, after each indexing round delete data more than `N` blocks behind the newest indexed block, giving a rolling N-block window (see `purge-db` for the exact cutoff). Requires `--live`; `--keep` without `--live` is an error, and `--keep 0` is rejected (use `purge-db --keep 0` to wipe). A one-time purge also runs right after the initial backfill.
- **`--max-range N`** - reject a backfill whose range is larger than `N` blocks, a guard against accidentally requesting a huge range.
- **`--batch-size N`** - how many blocks are fetched per batch (default `100`).

> **Archive data and free RPCs.** Free public RPC endpoints often do not retain archive data, so they cannot serve transactions from blocks more than a short distance behind the head (historical backfills against them will fail or return gaps). You can still build up a useful local store incrementally with free endpoints: run `index --live` to capture blocks as they are produced, so the data is fetched while it is still within the endpoint's retention window and cached locally from then on. For one-off historical backfills you will need an archive-capable endpoint (see [config.toml](./config.md)).

## Passing multiple RPC URLs

`--rpc-url` is repeatable. Passing it more than once spreads the batch fetch across every endpoint, which speeds up large backfills when a single provider rate-limits you or is the bottleneck.

```bash
# Fan the backfill out across three endpoints
mevlog index -b 22030800:22040800 --chain-id 1 \
  --rpc-url https://rpc-a.example \
  --rpc-url https://rpc-b.example \
  --rpc-url https://rpc-c.example
```

- **Parallel fetch, single writer.** Each batch of blocks is fetched concurrently, one process per URL round-robined across the endpoints, while writes to the local SQLite store stay single-writer. More endpoints means more fetch throughput without contending on the DB.
- **First URL is primary.** The first `--rpc-url` backs everything single-endpoint: the alloy provider, chain-head resolution, and chain-id verification. The rest are used only for concurrent block fetching. Ordering only matters in that the first is the one used for non-fetch RPC calls.
- **A single `--rpc-url` is unchanged.** With one endpoint the original sequential fetch-then-persist loop runs, so there is no behavior change unless you actually pass the flag twice or more.

> **All endpoints must be the same chain.** Every secondary URL is checked against the primary's chain ID before any indexing runs; if one reports a different chain, the command aborts rather than fetch foreign blocks and mark them indexed in the primary chain's store. Point every `--rpc-url` at the same network. Pass `--skip-verify-chain-id` to bypass the check (only when you are certain the endpoints match - a misconfigured URL or proxy will otherwise silently corrupt the local DB).

The same fan-out applies to `query`'s implicit indexing and every display command - repeat `--rpc-url` there too when resolving a large range.

## `reindex` command

Indexing can leave gaps: a transient RPC or network error during a backfill can cause individual blocks within the requested range to be skipped. `reindex` fills those holes.

```bash
mevlog reindex --chain-id 1
```

- It reads the stored block range (`MIN`/`MAX` indexed block) from the DB and re-runs indexing over that whole span.
- Because indexing only fetches blocks that are absent from the `blocks` table, a fully contiguous range is a no-op (`new_blocks = 0`) - only the missing blocks are refetched.
- This makes it safe to run repeatedly, or on a schedule, to heal a store that accumulated gaps from flaky network conditions.

## `purge-db` command

Removes old data to cap disk usage, keeping only a recent window.

```bash
# Keep the newest 1000 blocks, drop everything older
mevlog purge-db --keep 1000 --chain-id 1
```

- **`--keep N`** - keep blocks within `N` of the newest indexed block; rows with `block_number < MAX(block_number) - N + 1` are deleted from `logs`, `transactions`, `blocks`, and every tracked custom table in a single transaction. The newest indexed block in the local DB is the reference, so no RPC call is made. `--keep 0` purges everything.
- **`--reclaim`** - run `VACUUM` afterwards to actually shrink the file on disk. Off by default: freed pages are reused by later inserts, and `VACUUM` needs an exclusive whole-DB lock that can block concurrent readers/writers. This is why `index --live --keep` purges without reclaiming each round.

## `db-info` command

Reports the local store's indexed block range, row counts, file size, and any gaps.

```bash
mevlog db-info --chain-id 1
```

Sample output (from the [SQL demo](/search) store):

```json
{
  "chain_id": 1,
  "db_path": "/root/.mevlog/mevlog-txs-v1-1.db",
  "schema_version": 1,
  "db_size": "30.04 GB",
  "db_size_bytes": 32258461696,
  "wal_size_bytes": 468147392,
  "blocks": 50607,
  "transactions": 15493741,
  "logs": 44006981,
  "min_block": 25264887,
  "max_block": 25315493,
  "min_block_timestamp": 1780827719,
  "max_block_timestamp": 1781437283,
  "min_block_time": "2026-06-07 10:21:59 UTC",
  "max_block_time": "2026-06-14 11:41:23 UTC",
  "missing_blocks": 0
}
```

| Field | Meaning |
| --- | --- |
| `chain_id` | Chain ID of the local store. |
| `db_path` | Absolute path to the SQLite file. |
| `schema_version` | Migration schema version of the txs DB. |
| `db_size` / `db_size_bytes` | File size on disk, human-readable and in bytes. |
| `wal_size_bytes` | Size of the write-ahead log (`-wal`) sidecar file. |
| `blocks` / `transactions` / `logs` | Row counts in each table. |
| `min_block` / `max_block` | Lowest and highest indexed block numbers. |
| `min_block_timestamp` / `max_block_timestamp` | Unix timestamps of those blocks. |
| `min_block_time` / `max_block_time` | Same timestamps rendered as UTC. |
| `missing_blocks` | Count of blocks within `[min_block, max_block]` that have no row (gaps; `reindex` fills these). |
