ssh "$TARGET_NODE" << 'EOF'
for session in $(screen -ls | grep "mevlog-cron" | awk '{print $1}'); do
    echo "Terminating session $session"
    screen -X -S "$session" quit
done
echo "Stopped mevlog-cron"
EOF
