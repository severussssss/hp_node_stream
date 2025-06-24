#!/bin/bash

# Create test data directory
mkdir -p /home/ubuntu/node/orderbook-service/test_data/node_order_statuses

# Stream BTC orders
echo "Streaming BTC orders from Docker container..."
docker exec hyperliquid-node-1 tail -f /home/hluser/hl/data/node_order_statuses/order_status_0.bin > /home/ubuntu/node/orderbook-service/test_data/node_order_statuses/order_status_0.bin &
BTC_PID=$!

# Stream HYPE orders
echo "Streaming HYPE orders from Docker container..."
docker exec hyperliquid-node-1 tail -f /home/hluser/hl/data/node_order_statuses/order_status_159.bin > /home/ubuntu/node/orderbook-service/test_data/node_order_statuses/order_status_159.bin &
HYPE_PID=$!

echo "Streaming started. PIDs: BTC=$BTC_PID, HYPE=$HYPE_PID"
echo "Press Ctrl+C to stop..."

# Wait for interrupt
trap "kill $BTC_PID $HYPE_PID; echo 'Stopped streaming.'; exit" INT
wait