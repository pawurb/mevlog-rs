# Commands

These commands replay a single transaction through the EVM to surface execution-level details that are not stored in the indexed txs DB. Each takes a `<TX_HASH>` and an optional `--evm-trace <MODE>` flag selecting how the tx is traced:

- `revm` - local replay against a forked state DB (no special RPC support needed).
- `rpc` - `debug_traceTransaction` on the node (requires a debug-tracing RPC; check with `mevlog debug-available`).

### evm-coinbase-transfer

Compute a tx's direct ETH payment to its block's coinbase.

Replays the transaction and reports the value transferred straight to the block's coinbase (the miner/validator) during execution. This is the builder bribe MEV bundles use to pay for inclusion on top of the normal gas fee, so a non-zero amount is a strong signal that the tx is part of an MEV bundle. Returns the transferred amount.

```bash
mevlog evm-coinbase-transfer 0x8a3aab195d195afc0494bc030a98444ef591bf1a0728af8261dc613e53462768 \
  --chain-id 1 --evm-trace rpc
```

```json
{
  "tx_hash": "0x8a3aab195d195afc0494bc030a98444ef591bf1a0728af8261dc613e53462768",
  "coinbase": "0x4838b106fce9647bdf1e7877bf73ce8b0bad5f97",
  "amount_wei": "184500000000000000",
  "amount_eth": 0.1845
}
```

### evm-affected-addresses

List addresses affected by a tx.

Traces the transaction and returns every address it touches - the sender and recipient, contracts called along the trace, and accounts whose state was read or written. Useful for mapping the blast radius of a tx, spotting which protocols/tokens it interacts with, or feeding the address set into further queries.

```bash
mevlog evm-affected-addresses 0x8a3aab195d195afc0494bc030a98444ef591bf1a0728af8261dc613e53462768 \
  --chain-id 1 --evm-trace rpc
```

```json
[
  "0x000000000004444c5dc75cb358380d2e3de08a90",
  "0x1f9840a85d5af5bf1d1762f925bdaddc4201f984",
  "0x6b175474e89094c44da98b954eedeac495271d0f",
  "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
  "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
  "0xdac17f958d2ee523a2206206994597c13d831ec7"
]
```

### evm-state-diff

Show the storage state diff produced by a tx.

Replays the transaction and reports, per contract, every storage slot it changed with the before and after values (`NULL` when a slot was newly created or cleared). This is the raw on-chain effect of the tx - balance updates, reserve changes, ownership flips - at the storage-slot level. Output is grouped by contract address; each slot maps to a `[before, after]` pair.

```bash
mevlog evm-state-diff 0x8a3aab195d195afc0494bc030a98444ef591bf1a0728af8261dc613e53462768 \
  --chain-id 1 --evm-trace rpc
```

```json
{
  "0x0c1c1c109fe34733fca54b82d7b46b75cfb71f6e": {
    "0x4a05463a78312850be9f5852470d8add4dd90d2b371eb632b63ff1427e9a8eb4": [
      "0x000000000000000000000000000000000000000000001a8519aa2224e5dcb2a8",
      "0x000000000000000000000000000000000000000000001af3bbc528b7c12cdf6c"
    ],
    "0x8bf15480ba7f8fb71030709347201c97e06759a7a612696c84124306cd908280": [
      "0x00000000000000000000000000000000000000000000eeb8ef5740a6730874b8",
      "0x00000000000000000000000000000000000000000000ee4a4d3c3a1397b847f4"
    ]
  }
}
```

### evm-traces

Extract a tx's decoded call traces.

Traces the transaction and returns its decoded call tree: for each internal call, the `from`/`to` addresses and the resolved function signature (with its 4-byte selector hash when known; `?` when the selector is not in the signatures DB). Signatures are decoded against the local signatures DB. Useful for understanding exactly what a tx did step by step - which contracts it called and with which methods.

```bash
mevlog evm-traces 0x8a3aab195d195afc0494bc030a98444ef591bf1a0728af8261dc613e53462768 \
  --chain-id 1 --evm-trace rpc
```

```json
[
  {
    "from": "0xbdb3ba9ffe392549e1f8658dd2630c141fdf47b6",
    "to": "0x6546055f46e866a4b9a4a13e81273e3152bae5da",
    "signature": "swap(address,bool,int256,uint160,bytes)",
    "signature_hash": "0x128acb08"
  },
  {
    "from": "0x6546055f46e866a4b9a4a13e81273e3152bae5da",
    "to": "0x68749665ff8d2d112fa859aa293f07a622782f38",
    "signature": "transfer(address,uint256)",
    "signature_hash": "0xa9059cbb"
  },
  {
    "from": "0x6546055f46e866a4b9a4a13e81273e3152bae5da",
    "to": "0xdac17f958d2ee523a2206206994597c13d831ec7",
    "signature": "balanceOf(address)",
    "signature_hash": "0x70a08231"
  }
]
```
