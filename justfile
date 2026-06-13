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
    start_server() { export DEPLOYED_AT=$(date +%s); ./timestamp_assets.sh; cargo run --bin server & echo $! > /tmp/mevlog-server.pid; }; \
    trap 'stop_server; exit 0' INT TERM; \
    start_server; \
    fswatch -o --latency 1 -e ".*" -i "\.rs$" -i "\.html$" -i "\.css$" -i "\.js$" -i "\.jsx$" src templates styles javascripts/react javascripts/scripts.js | while read -r _; do \
        while read -r -t 1 _; do :; done; \
        echo "Change detected, rebuilding assets and restarting server..."; \
        stop_server; \
        start_server; \
    done

# Deploy backend using the deployment script
deploy:
    cd {{backend_dir}} && ./deploy.sh

# Deploy backend and restart
release:
    cd {{backend_dir}} && ./deploy.sh && ./remote/restart.sh

# Tail remote server and scheduler logs
logs target_node=env_var('TARGET_NODE'):
    cd {{backend_dir}} && TARGET_NODE={{target_node}} ./remote/logs.sh

# Pull latest and reinstall the CLI on the remote node
update-remote-cli target_node=env_var('TARGET_NODE'):
    ssh {{target_node}} 'cd ~/mevlog-rs && git pull && . "$HOME/.cargo/env" && OPENSSL_DIR=/usr/ OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu/ cargo install mevlog --path crates/mevlog --features mcp'

# Start the MCP server on the remote node (screen session, reads .env)
mcp-start target_node=env_var('TARGET_NODE'):
    cd {{backend_dir}} && TARGET_NODE={{target_node}} ./remote/mcp_start.sh

# Stop the MCP server on the remote node
mcp-stop target_node=env_var('TARGET_NODE'):
    cd {{backend_dir}} && TARGET_NODE={{target_node}} ./remote/mcp_stop.sh

# Show MCP server status on the remote node
mcp-status target_node=env_var('TARGET_NODE'):
    cd {{backend_dir}} && TARGET_NODE={{target_node}} ./remote/mcp_status.sh

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
