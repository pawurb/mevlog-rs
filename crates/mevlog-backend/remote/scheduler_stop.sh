#!/bin/bash
ssh "$TARGET_NODE" "systemctl stop mevlog-scheduler && echo 'Stopped mevlog-scheduler'"
