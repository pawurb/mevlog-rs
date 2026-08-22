#!/bin/bash
ssh "$TARGET_NODE" "systemctl start mevlog-server && systemctl status mevlog-server --no-pager | head -n 5"
