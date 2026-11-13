#!/bin/bash
# Follow journald logs of the server unit.
ssh -t "$TARGET_NODE" "journalctl -f -u mevlog-server"
