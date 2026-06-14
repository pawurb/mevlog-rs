# TUI Dashboard

`mevlog` ships with a full-blown chains explorer terminal UI. It is gated behind the `tui` feature, so install it with:

```bash
cargo install mevlog --features=tui --locked
```

and run:

```bash
mevlog tui
```

The dashboard lets you explore over 2k different EVM chains directly from your terminal, thanks to the [ChainList](https://chainlist.org/) integration. It uses the same local SQLite store as the CLI for data storage, so blocks fetched in the TUI are cached and reused by other commands.

![mevlog TUI interface](./images/quick-start/tui-screenshot.png)
