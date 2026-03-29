# MEV Log Backend

Backend crate for [mevlog-rs](https://github.com/pawurb/mevlog-rs) hosted at [mevlog.rs](https://mevlog.rs).

This Rust-based backend provides a web interface for querying EVM-compatible chains transactions.

## Architecture

The application consists of two main binaries:

- **Server** (`bin/server.rs`): Axum-based web server with REST endpoints and WebSocket connections
- **Scheduler** (`bin/scheduler.rs`): Background service for scheduled tasks and blockchain data collection

### Tech Stack

- **Axum** - Web framework with WebSocket support
- **mevlog** - Workspace dependency for MEV analysis functionality
- **Alloy** - Ethereum RPC interactions
- **REVM** - EVM execution and inspection
- **Askama** - HTML templating
- **Tokio-cron-scheduler** - Background job scheduling

## Development

### Building and Running

```bash
# Build the project
cargo build

# Run the web server (default port 3000)
just server

# Run the scheduler
cargo run --bin scheduler
```

### Testing and Linting

```bash
# Run tests
cargo test

# Format and lint
cargo fmt
cargo clippy
```

## Deployment

Deployment is automated using Ansible playbooks in `playbooks/` with nginx configuration.

```bash
./deploy.sh
```
