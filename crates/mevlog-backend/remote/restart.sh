ssh $TARGET_NODE << EOF
for session in \$(screen -ls | grep "mevlog-http" | awk '{print \$1}'); do
    echo "Terminating session \$session"
    screen -X -S "\$session" quit
done

for session in \$(screen -ls | grep "mevlog-cron" | awk '{print \$1}'); do
    echo "Terminating session \$session"
    screen -X -S "\$session" quit
done

EOF

ssh "$TARGET_NODE" << 'EOF'
screen -d -m -S mevlog-http bash -c 'export PATH="$HOME/.foundry/bin:$PATH" && cd /root/mevlog-backend/ && source .env && ./server > dbg.log 2>&1'
screen -d -m -S mevlog-cron bash -c 'export PATH="$HOME/.foundry/bin:$PATH" && cd /root/mevlog-backend/ && source .env && ./scheduler >> dbg.log 2>&1'
EOF
