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

Single-instance systemd deploy (`mevlog-server` unit, `EnvironmentFile=.env`, journald
logs; nginx config and unit installed by the Ansible playbook in `playbooks/`). A release
restarts the server in place, so expect a ~1s gap.

```bash
just release   # build, rsync, restart unit (health-checked on /uptime)
just logs      # follow journald logs
```
