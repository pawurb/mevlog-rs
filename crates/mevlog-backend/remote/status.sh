#!/bin/bash
ssh "$TARGET_NODE" "systemctl status mevlog-server mevlog-scheduler --no-pager"
