ssh $TARGET_NODE "cd /root/mevlog-backend; > varlogs/scheduler.log;"
ssh $TARGET_NODE "cd /root/mevlog-backend; > varlogs/server.log;"
ssh $TARGET_NODE "cd /root/mevlog-backend; > varlogs/dbg.log;"
echo "Cleared logs for: $TARGET_NODE"
