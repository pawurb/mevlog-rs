ssh "$TARGET_NODE" << 'EOF'
for session in $(screen -ls | grep "mevlog-http" | awk '{print $1}'); do
    echo "Stopping existing session $session"
    screen -X -S "$session" quit
done

screen -d -m -S mevlog-http bash -c 'export PATH="$HOME/.foundry/bin:$PATH" && cd /root/mevlog-backend/ && source .env && ./server > dbg.log 2>&1'
echo "Started mevlog-http"
screen -ls | grep mevlog-http || true
EOF
