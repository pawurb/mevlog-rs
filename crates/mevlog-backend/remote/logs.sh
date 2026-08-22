#!/bin/bash
# Follow journald logs of both units. Pass `server` or `scheduler` to follow only one.
case "${1:-}" in
    server)    ssh -t "$TARGET_NODE" "journalctl -f -u mevlog-server" ;;
    scheduler) ssh -t "$TARGET_NODE" "journalctl -f -u mevlog-scheduler" ;;
    *)         ssh -t "$TARGET_NODE" "journalctl -f -u mevlog-server -u mevlog-scheduler" ;;
esac
