backend_dir := "crates/mevlog-backend"

# Default recipe
default:
    @just --list

# Start the backend server with asset timestamping and environment setup
server:
    cd {{backend_dir}} && export DEPLOYED_AT=$(date +%s) && ./timestamp_assets.sh && cargo run --bin server

# Watch backend sources and auto-restart the server on changes
watch-server:
    cd {{backend_dir}} && \
    stop_server() { [ -f /tmp/mevlog-server.pid ] && kill "$(cat /tmp/mevlog-server.pid)" 2>/dev/null; pkill -f 'target/debug/server' 2>/dev/null; true; }; \
    start_server() { export DEPLOYED_AT=$(date +%s); export PORT=${PORT:-3002}; ./timestamp_assets.sh; cargo run --bin server & echo $! > /tmp/mevlog-server.pid; }; \
    rebuild_docs() { mdbook clean docs_src && mdbook build docs_src && cargo run --quiet --bin clean-html-links docs_html && echo "Docs rebuilt"; }; \
    watch_docs() { fswatch -o --latency 1 docs_src/src docs_src/book.toml docs_src/theme | while read -r _; do \
        while read -r -t 1 _; do :; done; \
        echo "Doc change detected, rebuilding docs..."; \
        rebuild_docs; \
    done; }; \
    stop_all() { stop_server; pkill -f 'fswatch.*docs_src' 2>/dev/null; true; }; \
    trap 'stop_all; exit 0' INT TERM; \
    start_server; \
    watch_docs & \
    fswatch -o --latency 1 -e ".*" -i "\.rs$" -i "\.html$" -i "\.css$" -i "\.js$" src templates styles javascripts/scripts.js | while read -r _; do \
        while read -r -t 1 _; do :; done; \
        echo "Change detected, rebuilding assets and restarting server..."; \
        stop_server; \
        start_server; \
    done

# Run benchmarks comparing two git refs
compare before after:
    bash scripts/compare.sh {{before}} {{after}}

# Re-download the GitHub stars badge with the current star count
refresh-stars:
    cd {{backend_dir}} && \
    stars=$(curl -sf https://api.github.com/repos/pawurb/mevlog-rs | jq -r .stargazers_count) && \
    curl -sf "https://img.shields.io/badge/Stars-${stars}-blue?style=social&logo=github" -o media/github-stars.svg && \
    mkdir -p assets && cp media/github-stars.svg assets/github-stars.svg && \
    echo "Badge updated: ${stars} stars"

# --- Production: delegated to the private infra repo (override its path with INFRA_DIR) ---
infra := env("INFRA_DIR", home_directory() / "Programming/infra")

# Build, upload and activate a production release
release:
    just -f {{infra}}/justfile release mevlog

# Build and upload a production release without activating it
deploy:
    just -f {{infra}}/justfile deploy mevlog

# Activate a production release (default: the newest uploaded)
activate release="":
    just -f {{infra}}/justfile activate mevlog {{release}}

# Roll production back to the previous release
rollback:
    just -f {{infra}}/justfile rollback mevlog

# List production releases (* = current)
releases:
    just -f {{infra}}/justfile releases mevlog

# Production logs, e.g. `just logs -f`
logs *args:
    just -f {{infra}}/justfile logs mevlog {{args}}
