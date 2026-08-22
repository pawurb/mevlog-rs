#!/bin/bash
set -euo pipefail

# Restart both systemd units, then wait for the server's /uptime to respond.
# A failed health check prints the unit status and exits non-zero.
ssh "$TARGET_NODE" 'bash -s' << 'EOF_REMOTE'
set -euo pipefail
source /root/mevlog-backend/.env
systemctl restart mevlog-scheduler
systemctl restart mevlog-server

for i in $(seq 1 40); do
    if curl -fsS "http://127.0.0.1:${PORT:-3000}/uptime" >/dev/null 2>&1; then
        echo "mevlog-server healthy on port ${PORT:-3000}"
        exit 0
    fi
    sleep 0.25
done
echo "mevlog-server FAILED health check" >&2
systemctl status mevlog-server --no-pager -l | tail -n 20 >&2 || true
exit 1
EOF_REMOTE
