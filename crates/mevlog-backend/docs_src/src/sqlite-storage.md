# Storage Requirements

The local txs DB grows with the amount of data you index. Each chain has its own file (`mevlog-txs-v1-{chain_id}.db`), plus a `-wal` / `-shm` sidecar during writes. Check actual usage at any time with `mevlog db-info --chain-id <id>`.

## Mainnet sizing estimates

Estimated from a contiguous week of Ethereum mainnet (~0.28 MB/block of raw data, ~307 txs and ~869 logs per block). Logs dominate - roughly 80% of the data. Your numbers will vary with traffic.

| Horizon | Blocks | Data only | On-disk (typical) |
| --- | ---: | ---: | ---: |
| 1 day | 7,200 | ~2 GB | ~4.5 GB |
| 1 month | 216,000 | ~63 GB | ~140 GB |
| 1 year | 2,628,000 | ~0.8 TB | ~1.7 TB |

Plan for **~1.5-1.7 TB per year** of disk if you index all of Mainnet and keep indexes. Most chains are far lighter, and you rarely need the full chain - index only the ranges you query.

## Reducing storage usage

- `index --live --keep N` holds a rolling window of the newest `N` blocks.
- `purge-db --keep N --chain-id <id>` drops older data on demand; add `--reclaim` to `VACUUM` and actually shrink the file (otherwise freed pages are reused, not returned to the OS).

## Indexes

`mevlog` ships with only a couple of minimal indexes (e.g. on `tx_hash`). **Broad indexes are deliberately not created by default** - they would inflate the storage size.

If you run heavy queries over a large store and notice slowdowns, add indexes that match your query patterns. The `query` connection is read-only, so create them by opening the file directly with the `sqlite3` CLI:

```bash
sqlite3 ~/.mevlog/mevlog-txs-v1-1.db \
  "CREATE INDEX idx_logs_address ON logs (address, block_number);"
```

Index what your `WHERE` clauses filter on (a log `address`, a `from_address`, etc.). The trade-off is always the same: faster reads - larger file.
