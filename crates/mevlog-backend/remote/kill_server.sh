#!/bin/bash
# Stop the server unit and show its state.
ssh "$TARGET_NODE" "systemctl stop mevlog-server; systemctl list-units 'mevlog-*' --all --no-pager"
