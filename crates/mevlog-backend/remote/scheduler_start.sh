ssh "$TARGET_NODE" << 'EOF'
for session in $(screen -ls | grep "mevlog-cron" | awk '{print $1}'); do
    echo "Stopping existing session $session"
    screen -X -S "$session" quit
done

screen -d -m -S mevlog-cron bash -c 'export PATH="$HOME/.foundry/bin:$PATH" && cd /root/mevlog-backend/ && source .env && ./scheduler >> dbg.log 2>&1'
echo "Started mevlog-cron"
screen -ls | grep mevlog-cron || true
EOF
