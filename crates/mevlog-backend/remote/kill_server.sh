#!/bin/bash
# Stop both units and show their state.
ssh "$TARGET_NODE" "systemctl stop mevlog-server mevlog-scheduler; systemctl list-units 'mevlog-*' --all --no-pager"
