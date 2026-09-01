#!/bin/bash
set -euo pipefail

bash timestamp_assets.sh

(cd docs_src && mdbook clean && mdbook build)
cargo run --bin clean-html-links docs_html

cross build --release --target x86_64-unknown-linux-musl --features hotpath,hotpath-alloc,hotpath-meta

# Ensure app dir exists, then rsync binaries
ssh $TARGET_NODE mkdir -p /root/mevlog-backend
rsync -avz ../../target/x86_64-unknown-linux-musl/release/server $TARGET_NODE:/root/mevlog-backend/server
rsync -avz ../../target/x86_64-unknown-linux-musl/release/scheduler $TARGET_NODE:/root/mevlog-backend/scheduler

rsync -azr --delete templates/ $TARGET_NODE:/root/mevlog-backend/templates
rsync -azr --delete assets/ $TARGET_NODE:/root/mevlog-backend/assets
rsync -azr --delete media/ $TARGET_NODE:/root/mevlog-backend/media
rsync -azr --delete docs_html/ $TARGET_NODE:/root/mevlog-backend/docs_html
# Remote env file: plain KEY=value lines (systemd EnvironmentFile), no `export`
rsync -av ../../.env-remote $TARGET_NODE:/root/mevlog-backend/.env
