#!/bin/bash
set -euo pipefail

# Restart the two systemd units (mevlog-server, mevlog-scheduler). Single
# server instance, so there is a short gap while it comes back up (nginx
# 502s for ~1s).
#
# The server is restarted first and polled on /uptime. A failed health check
# aborts before touching the scheduler (the old scheduler process keeps
# running) and dumps the unit status.
ssh "$TARGET_NODE" 'bash -s' << 'EOF_REMOTE'
set -euo pipefail
cd /root/mevlog-backend
port="$(grep -E '^PORT=' .env | cut -d= -f2- | sed -E "s/^[\"']//; s/[\"']$//")"
port="${port:-3000}"

systemctl restart mevlog-server

healthy=0
for i in $(seq 1 40); do
    if curl -fsS "http://127.0.0.1:$port/uptime" >/dev/null 2>&1; then
        healthy=1
        break
    fi
    sleep 0.25
done

if [ "$healthy" -ne 1 ]; then
    echo "mevlog-server FAILED health check on port $port" >&2
    systemctl status mevlog-server --no-pager -l | tail -n 20 >&2 || true
    exit 1
fi
echo "mevlog-server healthy on port $port"

systemctl restart mevlog-scheduler
sleep 1
if ! systemctl is-active --quiet mevlog-scheduler; then
    echo "mevlog-scheduler FAILED to start" >&2
    systemctl status mevlog-scheduler --no-pager -l | tail -n 20 >&2 || true
    exit 1
fi
echo "mevlog-scheduler active"
EOF_REMOTE
echo "Restart complete"
