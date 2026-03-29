backend_dir := "crates/mevlog-backend"

# Default recipe
default:
    @just --list

# Start the backend server with asset timestamping and environment setup
server:
    cd {{backend_dir}} && ./timestamp_assets.sh && cargo run --bin server

# Deploy backend using the deployment script
deploy:
    cd {{backend_dir}} && ./deploy.sh

# Deploy backend and restart
release:
    cd {{backend_dir}} && ./deploy.sh && ./remote/restart.sh
