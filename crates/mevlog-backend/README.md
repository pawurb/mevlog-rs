# MEV Log Backend

Backend crate serving [mevlog.rs](https://mevlog.rs): the landing page and the documentation for [mevlog-rs](https://github.com/pawurb/mevlog-rs).

## Architecture

- **Server** (`bin/server.rs`): Axum web server rendering the landing page (Askama templates) and serving the mdBook docs from `docs_html/`.

## Development

```bash
# Run the web server (timestamps assets and builds docs)
just server

# Tests, format, lint
cargo test -p mevlog-backend
cargo fmt -p mevlog-backend
cargo clippy -p mevlog-backend
```

## Deployment

Production deployment (server setup, process management, secrets) is managed in a separate
private repository. This repo only builds the app: `just server` for local development, and
the production build is a static `x86_64-unknown-linux-musl` binary plus `templates/`, `assets/`,
`media/` and `docs_html/`. `timestamp_assets.sh` prefixes asset file names with `DEPLOYED_AT`,
which the server must receive with the same value at runtime.
