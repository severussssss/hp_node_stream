#!/usr/bin/env python3
import json
import struct
import os
import subprocess
import time
from datetime import datetime

# Market name to ID mapping
MARKET_IDS = {
    "BTC": 0,
    "ETH": 1, 
    "ARB": 2,
    "OP": 3,
    "MATIC": 4,
    "AVAX": 5,
    "SOL": 6,
    "ATOM": 7,
    "FTM": 8,
    "NEAR": 9,
    "HYPE": 159,
    "WIF": 10,  # Adding more markets as we see them
}

def get_market_id(coin):
    """Get market ID from coin name"""
    return MARKET_IDS.get(coin, -1)

def convert_to_binary(order_json):
    """Convert JSON order to binary format"""
    try:
        data = json.loads(order_json)
        
        # Extract fields
        coin = data["order"]["coin"]
        market_id = get_market_id(coin)
        
        # Skip unknown markets
        if market_id == -1:
            return None
            
        order_id = data["order"]["oid"]
        price = float(data["order"]["limitPx"])
        size = float(data["order"]["sz"])
        is_buy = 1 if data["order"]["side"] == "B" else 0
        timestamp_ns = data["order"]["timestamp"] * 1000000  # Convert ms to ns
        
        # Status mapping
        status_map = {"open": 0, "filled": 1, "canceled": 2, "cancelled": 2}
        status = status_map.get(data["status"], 0)
        
        # Pack to binary (38 bytes)
        return struct.pack(
            '<QIddbQb',
            order_id,
            market_id,
            price,
            size,
            is_buy,
            timestamp_ns,
            status
        )
    except Exception as e:
        return None

def main():
    # Create output directory
    output_dir = "/home/ubuntu/node/orderbook-service/live_data"
    os.makedirs(output_dir, exist_ok=True)
    
    # Open output files for tracked markets
    market_files = {}
    tracked_markets = {0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 159}  # Top 10 + HYPE
    
    for market_id in tracked_markets:
        filename = f"{output_dir}/order_status_{market_id}.bin"
        market_files[market_id] = open(filename, "wb")
    
    print(f"Streaming real-time orders to {len(tracked_markets)} market files...")
    
    # Get current hour file
    current_hour = datetime.now().hour
    current_date = datetime.now().strftime("%Y%m%d")
    source_file = f"/home/hluser/hl/data/node_order_statuses/hourly/{current_date}/{current_hour}"
    
    print(f"Reading from: {source_file}")
    
    # Start tailing the file
    cmd = ["docker", "exec", "hyperliquid-node-1", "tail", "-f", source_file]
    process = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    
    order_count = 0
    start_time = time.time()
    
    try:
        for line in iter(process.stdout.readline, b''):
            line = line.decode('utf-8').strip()
            if not line:
                continue
                
            # Convert to binary
            binary_data = convert_to_binary(line)
            if binary_data:
                # Extract market ID
                market_id = struct.unpack('<I', binary_data[8:12])[0]
                
                # Write to appropriate file
                if market_id in market_files:
                    market_files[market_id].write(binary_data)
                    market_files[market_id].flush()
                    order_count += 1
                    
                    # Log progress
                    if order_count % 1000 == 0:
                        elapsed = time.time() - start_time
                        rate = order_count / elapsed
                        print(f"Processed {order_count} orders, {rate:.0f} orders/sec")
                        
    except KeyboardInterrupt:
        print("\nStopping...")
    finally:
        process.terminate()
        for f in market_files.values():
            f.close()
        print(f"Streamed {order_count} orders total")

if __name__ == "__main__":
    main()