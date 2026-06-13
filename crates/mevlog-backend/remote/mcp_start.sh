ssh "$TARGET_NODE" << 'EOF'
for session in $(screen -ls | grep "mevlog-mcp" | awk '{print $1}'); do
    echo "Stopping existing session $session"
    screen -X -S "$session" quit
done

screen -d -m -S mevlog-mcp bash -c 'export PATH="$HOME/.cargo/bin:$PATH" && cd /root/mevlog-backend/ && source .env && mevlog mcp --rpc-url="$REMOTE_ETH_RPC_URL" > mcp.log 2>&1'
echo "Started mevlog-mcp"
screen -ls | grep mevlog-mcp || true
EOF
