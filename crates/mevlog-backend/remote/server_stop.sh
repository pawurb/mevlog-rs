#!/bin/bash
ssh "$TARGET_NODE" "systemctl stop mevlog-server && echo 'Stopped mevlog-server'"
