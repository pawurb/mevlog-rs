# Storage Requirements

> How much disk the local txs DB uses, and how to add your own indexes for
> faster queries.

## Disk usage

- TODO: data scales with indexed blocks. Rough Ethereum mainnet figure:
  ~0.3 MB per block (≈380 txs + ≈840 logs each).
- TODO: rules of thumb - 1,000 blocks ≈ 0.3 GB; one day of mainnet
  (~7,200 blocks) ≈ 2 GB; one week ≈ 15 GB. Lighter chains use far less.
- TODO: each chain has its own file (`mevlog-txs-v1-{chain_id}.db`); WAL
  (`-wal`) and shared-memory (`-shm`) sidecar files appear during writes.
- TODO: check actual usage with `mevlog db-info`.

## Keeping size down

- TODO: `--keep N` prunes to a rolling N-block window during `index --live`.
- TODO: `purge-db` removes data below a block window.

## Adding indexes

- TODO: built-in indexes - `idx_transactions_hash`, `idx_blocks_timestamp`.
- TODO: the `query` connection is read-only, so add indexes by opening the file
  directly with the `sqlite3` CLI, e.g.
  `sqlite3 ~/.mevlog/mevlog-txs-v1-1.db "CREATE INDEX ... "`.
- TODO: trade-off - indexes speed reads but grow the file and slow indexing.
