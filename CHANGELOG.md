# Changelog

All notable changes to this project will be documented in this file.

## [0.8.1] - 2026-01-31

### 🚀 Features

- Add evm state diff tracing
- Add TUI evm state diff

### ⚙️ Miscellaneous Tasks

- Release 0.8.1

## [0.8.0] - 2026-01-13

### 🚀 Features

- Add to_ens to txs json
- Validate latest block
- Initial TUI implementation
- Add TUI network selection
- Add opcodes tracing
- Add TUI tabs
- Initial tx inspect view
- Add initial tx view tabs
- Show opcodes in tx popup
- Show tx transfers
- Add blocks navigation
- Enable network reselection
- Show tx traces
- Add debug-available helper
- Config file and improve RPC management
- RPC info and network selection
- Initial txs search form
- Add tx trace binding
- Initial search form
- Display search results
- Custom mevlog cmd path
- Search UI improvements
- More TUI UI improvements

### 🐛 Bug Fixes

- Sort by erc20Transfer, only include txs with that token
- Use dedicated RPC for test
- Fix hotpath PR profile config
- Fix optimism opcodes tracing
- Display network name from RPC
- Only abort unfinished tasks
- Fix opcodes align
- Error and UI fixes
- Improve partial cryo caching
- Handle initial sqlite db setup
- Improve logging and dont expose rpc-url

### 🚜 Refactor

- Dry cleanup
- Exhaustive opcodes styling
- Reuse search logic
- Cleanup

### ⚡ Performance

- Reduce tx and chain clones
- Batch cryo data fetch
- Improve table rendering performance

### ⚙️ Miscellaneous Tasks

- Update hotpath
- Use multiple Rust versions in CI
- Configure hotpath CI
- Update hotpath
- Update hotpath, use measure_all macros
- Cargo update
- Adjust hotpath CI
- More secure hotpath CI setup
- Profile alloc bytes and count
- Update hotpath
- Update hotpath
- Update alloy
- Cargo update
- Add default config.toml
- Update and configure hotpath
- Release 0.8.0

## [0.7.1] - 2025-09-15

### 🚀 Features

- Add latest_offset config
- Display CREATE tx contract addr
- Add --native-token-price
- Add --sort by erc20Transfer amounts
- Add --max-range option

### 🐛 Bug Fixes

- Improve ENS filtering
- Correct json txs order
- Reuse sqlite connection
- Support chains without base fee
- Fix dependencies conflict
- Fix serde version

### 🚜 Refactor

- Simplify cryo files find

### ⚡ Performance

- Paraller metadata rpc calls
- Take best responding rpc urls
- Get_chain_id only if necessary
- Get latest block only if needed
- Setup hotpath benchmark

### ⚙️ Miscellaneous Tasks

- Enable revm integration CI
- Add maxperf and instrument profiles
- Add cargo audit to CI
- Use edition 2024
- Release mevlog version 0.7.1

## [0.7.0] - 2025-08-18

### 🚀 Features

- Add rpc-urls cmd
- Chain-id integration with ChainList
- Add cmd listing known chains
- Add json format
- Include all txs count in json
- Search from the newest block
- Add data to tx json
- Adjust json format for web UI
- Flatten json structure
- Add --sort and --limit
- Print json errors
- Add from_ens json field
- Add --ens and --erc20-symbols flags
- Json output for chains cmd

### 🐛 Bug Fixes

- Unify chains data sources
- Respect RUST_LOG config [#26]
- Adjust json data output
- Deterministic --sort
- Report cryo errors
- Use custom revm cache dir
- Fix revm forking and drop Anvil dependency
- Detect chain-id mismatch

### 🚜 Refactor

- Refactor init_deps
- Simplify signature overwrites
- Extract ConnOpts
- Unify generate_block method calling
- Rename vars
- Change modules config
- Use structs for chain-info json

### ⚡ Performance

- Use cryo for logs
- Cache block metadata
- Improve revm caching
- In-memory signatures cache
- Cache ChainList response
- Use cryo parquet instead of csv
- Memory cache for ens and symbols

### ⚙️ Miscellaneous Tasks

- Enable CI integration tests and cache dependencies
- More cli tests
- Rename rpc-urls to chain-info
- Release mevlog version 0.7.0

## [0.6.0] - 2025-07-17

### 🚀 Features

- Filtering & watching by subcalls (#17)
- Allow for working with chains that are not hardcoded yet (#18)
- Add chains info to sqlite database (#19)
- Support unsupported chains
- Add failed txs filter
- Add --transfer filter and display amount (#21)

### 🐛 Bug Fixes

- Fix Revm simulations and cache
- Dont revm simulate failed txs
- Enable and validate show-calls
- Fix non tracing filters

### 🚜 Refactor

- Refactor chain signature overrides
- Add complete cryo_cache_dir_name values
- Conn_opts to shared_opts

### ⚡ Performance

- Optimize hosted db size and seed performance
- Use zstd for sqlite compression

### ⚙️ Miscellaneous Tasks

- CI check dev feature flag
- Add sqlite db upload script
- Update alloy and Revm (#20)
- Disable nightly lint and unstable feature
- Rename --transfer to --erc20-transfer
- Add tokio-console support
- Release mevlog version 0.6.0

## [0.5.7] - 2025-06-18

### 🚀 Features

- Add ETH units
- Add fantom chain

### 🐛 Bug Fixes

- Fix revm tracing and csv parse logic

### ⚙️ Miscellaneous Tasks

- CI use nightly Rust
- Release mevlog version 0.5.7

## [0.5.6] - 2025-06-07

### 🚀 Features

- Add scroll chain
- Add filtering by value
- Display txs value
- Improve value display

### ⚡ Performance

- Reuse native token price

### ⚙️ Miscellaneous Tasks

- Add changelog
- Add crate-release changelog hook
- Release mevlog version 0.5.6

## [0.5.5] - 2025-05-27

<!-- generated by git-cliff -->
