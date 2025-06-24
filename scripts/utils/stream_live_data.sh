#!/bin/bash

# Stream live order data from Docker container to local files
echo "Starting live data streaming from Hyperliquid node..."

# Create named pipes for each market
mkdir -p /home/ubuntu/node/orderbook-service/live_data

# Get current hour
CURRENT_HOUR=$(date +%H | sed 's/^0//')
CURRENT_DATE=$(date +%Y%m%d)
SOURCE_FILE="/home/hluser/hl/data/node_order_statuses/hourly/${CURRENT_DATE}/${CURRENT_HOUR}"

echo "Streaming from: $SOURCE_FILE"

# Tail the live file and split by market
docker exec hyperliquid-node-1 tail -f "$SOURCE_FILE" | \
python3 -c "
import sys
import struct
import os

# Create output files for each market
market_files = {}
OUTPUT_DIR = '/home/ubuntu/node/orderbook-service/live_data'
os.makedirs(OUTPUT_DIR, exist_ok=True)

# Popular markets we want to track
MARKETS = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 159}  # Top 10 perps + HYPE

for market_id in MARKETS:
    filename = f'{OUTPUT_DIR}/order_status_{market_id}.bin'
    market_files[market_id] = open(filename, 'ab')

print(f'Streaming live orders to {len(MARKETS)} market files...')

ORDER_SIZE = 38
buffer = b''

while True:
    try:
        # Read from stdin in chunks
        chunk = sys.stdin.buffer.read(ORDER_SIZE * 100)
        if not chunk:
            break
            
        buffer += chunk
        
        # Process complete orders
        while len(buffer) >= ORDER_SIZE:
            order_data = buffer[:ORDER_SIZE]
            buffer = buffer[ORDER_SIZE:]
            
            # Parse market ID (bytes 8-12)
            market_id = struct.unpack('<I', order_data[8:12])[0]
            
            # Write to appropriate file if it's a market we track
            if market_id in market_files:
                market_files[market_id].write(order_data)
                market_files[market_id].flush()
                
    except KeyboardInterrupt:
        break
    except Exception as e:
        print(f'Error: {e}')
        continue

# Close all files
for f in market_files.values():
    f.close()
print('Stopped streaming.')
"