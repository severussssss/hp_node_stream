#!/bin/bash

echo "Monitoring order file sizes in Docker container..."

for i in {1..10}; do
    echo "=== Sample $i ==="
    echo -n "BTC orders: "
    docker exec hyperliquid-node-1 ls -la /home/hluser/hl/data/node_order_statuses/order_status_0.bin 2>/dev/null | awk '{print $5, $6, $7, $8}'
    echo -n "HYPE orders: "
    docker exec hyperliquid-node-1 ls -la /home/hluser/hl/data/node_order_statuses/order_status_159.bin 2>/dev/null | awk '{print $5, $6, $7, $8}'
    echo
    sleep 1
done