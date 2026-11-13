#!/bin/bash
set -euo pipefail

# Restart the mevlog-server systemd unit. Single server instance, so there is
# a short gap while it comes back up (nginx 502s for ~1s).
#
# The server is polled on /uptime; a failed health check dumps the unit status
# and exits 1. Also exits 1 if the retired mevlog-scheduler unit is still
# installed (remove it with the ansible playbook).
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

# The scheduler was retired; its unit is removed only by the ansible server
# role (units are never managed from the release path). Fail loudly so a
# release cannot silently leave the old indexer running.
if systemctl list-unit-files mevlog-scheduler.service --no-legend 2>/dev/null | grep -q mevlog-scheduler; then
    echo "Retired mevlog-scheduler unit is still installed." >&2
    echo "Run 'ansible-playbook playbooks/setup.yml' from crates/mevlog-backend to stop and remove it." >&2
    exit 1
fi
EOF_REMOTE
echo "Restart complete"
