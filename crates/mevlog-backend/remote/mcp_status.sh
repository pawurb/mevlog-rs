ssh "$TARGET_NODE" << 'EOF'
echo "=== screen session ==="
screen -ls | grep mevlog-mcp || echo "no mevlog-mcp screen session"
echo "=== process ==="
pgrep -fa 'mevlog mcp' || echo "no 'mevlog mcp' process"
echo "=== listening on :6671 ==="
ss -ltnp | grep ':6671' || echo "not listening on :6671"
EOF
