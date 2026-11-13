#!/bin/bash
ssh "$TARGET_NODE" "systemctl status mevlog-server --no-pager"
