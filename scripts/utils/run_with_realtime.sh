#!/bin/bash

echo "Starting orderbook service with real-time Hyperliquid data..."

# Create directory for real-time data
mkdir -p /home/ubuntu/node/orderbook-service/realtime_data

# Get current hour
HOUR=$(date +%H | sed 's/^0//')
DATE=$(date +%Y%m%d)

# Start streaming each market's data in background
for MARKET_ID in 0 1 2 3 4 5 6 7 8 9; do
    # Create named pipe for this market
    PIPE="/home/ubuntu/node/orderbook-service/realtime_data/order_status_${MARKET_ID}.json"
    rm -f "$PIPE"
    mkfifo "$PIPE"
    
    # Stream filtered data for this market
    docker exec hyperliquid-node-1 tail -f "/home/hluser/hl/data/node_order_statuses/hourly/${DATE}/${HOUR}" | \
    grep -E "\"coin\":\"(BTC|ETH|ARB|OP|MATIC|AVAX|SOL|ATOM|FTM|NEAR)\"" | \
    awk -v market=$MARKET_ID -v coin=$MARKET_ID '
    BEGIN {
        coins["BTC"]=0; coins["ETH"]=1; coins["ARB"]=2; coins["OP"]=3; coins["MATIC"]=4;
        coins["AVAX"]=5; coins["SOL"]=6; coins["ATOM"]=7; coins["FTM"]=8; coins["NEAR"]=9;
    }
    {
        if (match($0, /"coin":"([^"]+)"/, arr)) {
            if (coins[arr[1]] == market) print $0
        }
    }' > "$PIPE" &
done

# Give pipes time to initialize
sleep 2

# Run the optimized service
exec /home/ubuntu/node/orderbook-service/target/release/orderbook-service-optimized \
    --data-dir /home/ubuntu/node/orderbook-service/realtime_data \
    --grpc-port 50052 \
    --markets 0:BTC,1:ETH,2:ARB,3:OP,4:MATIC,5:AVAX,6:SOL,7:ATOM,8:FTM,9:NEAR