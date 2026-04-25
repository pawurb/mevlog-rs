---
name: mevlog-cli
description: Use the mevlog CLI to search and inspect EVM transactions, recent blocks, chain metadata, tracing output, and storage diffs on Ethereum and other EVM chains. Trigger when the user wants to find transactions by address, method, event, gas or value, ERC20 transfer, MEV pattern, validator bribe, contract deployment, or storage changes, or inspect a specific transaction or chain. Prefer this before writing custom RPC code. Do not use it for raw eth_call or eth_getLogs workflows, ABI generation, or general Foundry contract interaction.
---

# mevlog

Use `mevlog` when the task is "find transactions where X" on an EVM chain.

Prefer `mevlog` over custom RPC scripts when the user wants to:
- Search recent or historical transactions with multiple filters
- Inspect one transaction in detail
- Detect likely MEV activity or validator bribes
- Check whether a contract's storage changed
- Compare chains or discover working public RPCs

Use `cast` or direct RPC calls instead when the task is mainly `eth_call`, `eth_getLogs`, ABI work, or contract scripting.

## Pick the command

Choose the narrowest command that matches the task:

| Command | Use for |
|---|---|
| `mevlog search` | Search transactions across a block range with filters |
| `mevlog tx <hash>` | Inspect one mined transaction, optionally with surrounding txs |
| `mevlog chains` | List supported EVM chains |
| `mevlog chain-info --chain-id <id>` | Show chain details and benchmarked RPC URLs |
| `mevlog debug-available --rpc-url <url>` | Check whether an RPC supports `debug_traceTransaction` |

## Connect to a chain

Choose one connection mode per command:

- Prefer `--chain-id <id>` for ad hoc queries on public infrastructure.
- Use `--rpc-url <url>` for private or archive RPCs, repeated queries, or `--evm-trace rpc`.

Treat these chain IDs as common defaults: `1` Ethereum, `137` Polygon, `56` BSC, `42161` Arbitrum, `10` Optimism, `8453` Base.

Run `mevlog chains --filter <name>` if you need to discover another chain ID.

## Start narrow

Begin with the smallest query that can prove the idea:

- Use `-b 5:latest` or `-b 10:latest` before widening the range.
- Use `-p 0` or `-p 0:5` to focus on top-of-block activity before adding expensive filters.
- Add `--max-range <n>` to `search` as a safety cap so a mistyped block range cannot trigger a runaway query.
- Remember that in `mevlog tx`, `-b` and `-a` mean surrounding transaction counts, not block range, and each is capped at 5.
- The `-b`/`-a` semantics in `mevlog tx` are counterintuitive: `-b` ("before") returns txs with *smaller* indexes (earlier in the block, newer in mempool-arrival terms), and `-a` ("after") returns txs with *larger* indexes. If the user says "show the 3 txs that came before this one in block order," that maps to `-b 3`, but double-check against their intent before widening.

Public RPCs throttle quickly. Prove the filter first, then widen the search.

## Add filters

Compose filters incrementally:

1. Start with the cheapest narrowing signal such as block range, tx position, sender, receiver, or one event.
2. Add method, value, gas, or ERC20 transfer filters next.
3. Add tracing-dependent filters only if the task truly needs storage changes, internal calls, full transaction cost, opcodes, or state diffs.

Remember these rules:

- Filters AND together.
- `--event` is repeatable and all provided patterns must match.
- `--not-event` takes a single pattern (not repeatable) and subtracts matches. Use a regex like `/(Transfer|Approval).+/` if you need to exclude several signatures at once.
- Wrap regex patterns in `/.../`.

### Address filters

- `--from <addr|ens>`: Match sender address or ENS name such as `jaredfromsubway.eth`
- `--to <addr|ens|CREATE>`: Match receiver, ENS name, or the literal `CREATE` for contract deployments
- `--touching <addr>`: Match transactions that modified storage on this contract; requires `--evm-trace`

### Event filters

Use `--event` repeatedly to require multiple matches, and `--not-event` (once) to subtract. Each flag accepts one of:

- Bare contract address: `0x6982508145454ce325ddbe47a25d4ec3d2311933`
- Event signature: `Transfer(address,address,uint256)`
- Regex wrapped in slashes: `/(?i)(rebase).+/`
- Signature plus address: `Transfer(address,address,uint256)|0x6982...`

### Method filters

- `--method <pat>`: Match the root call by regex, full signature, or 4-byte selector
- `--calls <pat>`: Match internal sub-call methods; requires `--evm-trace`

Use `--method "<Unknown>"` to look for calls with no known signature. This is often a useful MEV heuristic.

### Value, cost, and gas filters

Use comparison operators `ge` and `le` plus a unit such as `ether`, `gwei`, or raw value:

- `--value ge1ether`
- `--tx-cost ge0.01ether`
- `--real-tx-cost ge0.02ether`
- `--gas-price ge2gwei`
- `--real-gas-price ge10gwei`

Treat `--real-tx-cost` and `--real-gas-price` as tracing-dependent because they include coinbase bribes.

### ERC20 transfer filters

Repeat `--erc20-transfer` as needed:

- `0xTOKEN`: Match any `Transfer` event for that token
- `0xTOKEN|ge1000gwei`: Match transfers above a threshold using token decimals

Add `--erc20-transfer-amount` when you want amounts shown inline in output.

### Output and display flags

- `--failed`: Show only failed transactions
- `--logs`: Include logs. Significantly inflates payload size on `search` queries — prefer running without `--logs` first, then re-run `mevlog tx <hash> --logs` only for the txs you need to inspect
- `--ens`: Resolve ENS names
- `--erc20-symbols`: Resolve token symbols
- `--evm-calls`: Show detailed call info; requires `--evm-trace`
- `--evm-ops`: Show executed opcodes; requires `--evm-trace`
- `--evm-state-diff`: Show storage slot changes; requires `--evm-trace`

### Sorting and limiting

- `--sort <field>`: `gas-price`, `gas-used`, `tx-cost`, `full-tx-cost`, or `erc20Transfer|<token_address>`
- `--sort-dir asc|desc`
- `--limit <n>`

Treat `--sort full-tx-cost` as tracing-dependent.

## Enable tracing deliberately

Add `--evm-trace` only when a filter or output mode requires it.

Choose one tracing backend:

- `--evm-trace revm`: Replay the block locally against a fork DB. Use this by default on public RPCs.
- `--evm-trace rpc`: Call `debug_traceTransaction`. Use this only when the RPC definitely exposes debug methods.

Prefer `revm` when:

- You are using public infrastructure
- You do not know whether `debug_*` is available
- Reliability matters more than raw speed

Prefer `rpc` when:

- The user supplied a private node or archive RPC
- `mevlog debug-available --rpc-url <url>` returned `true`
- You want faster tracing and trust the RPC implementation

Add `--evm-trace revm` or `--evm-trace rpc` before using:

- `--touching`
- `--calls`
- `--real-tx-cost`
- `--real-gas-price`
- `--evm-calls`
- `--evm-ops`
- `--evm-state-diff`
- `--sort full-tx-cost`

Assume tracing queries are more expensive than plain `search` queries:

- Start with a very small block range
- Combine tracing with narrow position filters where possible
- Expect public RPCs to rate-limit or slow down

## Common query patterns

Use these patterns as starting points.

**MEV bot activity in recent blocks**
```bash
mevlog search -b 10:latest -p 0:2 --method "<Unknown>" --chain-id 1
```

**Transactions interacting with a contract**
```bash
mevlog search -b 10:latest --event 0xCONTRACT --chain-id 1
```

**Transactions that changed a contract's storage**
```bash
mevlog search -b 10:latest --touching 0xCONTRACT --evm-trace revm --chain-id 1
```

**Large transfers of a token with amounts shown**
```bash
mevlog search -b 50:latest \
  --erc20-transfer "0xTOKEN|ge100000gwei" \
  --erc20-transfer-amount \
  --chain-id 1
```

**Most expensive transactions including bribes**
```bash
mevlog search -b 10:latest --sort full-tx-cost --limit 5 --evm-trace revm --chain-id 1
```

**Transactions from an ENS name**
```bash
mevlog search -b 20:latest --from vitalik.eth --chain-id 1
```

**Contract deployments**
```bash
mevlog search -b 20:latest --to CREATE --chain-id 1
```

**Swap events without Transfer events**
```bash
mevlog search -b 10:latest --event "/(Swap).+/" --not-event "/(Transfer).+/" --chain-id 1
```

**One transaction with opcodes and storage diff**
```bash
mevlog tx 0xHASH --chain-id 1 --evm-trace revm --evm-ops --evm-state-diff
```

**Transactions surrounding a target transaction**
```bash
mevlog tx 0xHASH -b 2 -a 2 --chain-id 1
```

## Format output for the task

`--format` is a global flag and must appear before the subcommand name, e.g. `mevlog --format json search ...`. Choose based on what you need next:

- The default is `json-pretty`; good for human inspection.
- Use `--format json` when piping into `jq` or another script.
- Expect `search` and `tx` to return a JSON object with `result`, `result_count`, `duration`, `chain`, and `query`.
- Expect `debug-available` to print a bare boolean (not JSON).
- Expect `chain-info` and `chains` to emit their own JSON structures.

## Work efficiently

Keep these operational habits in mind:

- Run a small query first if you plan to add `--ens` or `--erc20-symbols`; this warms caches under `~/.mevlog/`.
- Prefer a fixed `--rpc-url` inside loops because `--chain-id` benchmarks RPCs on each invocation.
- Use `--erc20-transfer-amount` when token amount display matters in the output.

## Install if missing

Install the CLI from crates.io if `mevlog` is not on `PATH`:

```bash
cargo install mevlog
```

Install `cryo_cli` as well:

```bash
cargo install cryo_cli
```

Expect the first run to download ChainList metadata and openchain signatures into `~/.mevlog/`. Read and edit `~/.mevlog/config.toml` if you need custom RPC URLs per chain.
