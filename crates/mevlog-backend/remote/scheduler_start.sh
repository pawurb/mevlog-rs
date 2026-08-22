#!/bin/bash
ssh "$TARGET_NODE" "systemctl start mevlog-scheduler && systemctl status mevlog-scheduler --no-pager | head -n 5"
