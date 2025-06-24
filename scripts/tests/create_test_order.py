#!/usr/bin/env python3
import struct
import time
import os

# Create a test order file with numeric name
timestamp = int(time.time())
temp_file = f"/tmp/{timestamp}.bin"

with open(temp_file, "wb") as f:
    base_time = int(time.time() * 1e9)
    
    # Write 5 buy orders
    for i in range(5):
        order_data = struct.pack('<QIddBQB',
            7000000 + i,        # order_id
            0,                  # market_id (BTC)
            94990.0 - i * 10,   # price
            1.0 + i * 0.2,      # size
            1,                  # is_buy
            base_time + i * 1000,  # timestamp_ns
            0                   # status (Open)
        )
        f.write(order_data)
        print(f"Buy order {i+1}: ${94990.0 - i * 10:.2f} x {1.0 + i * 0.2:.1f}")
    
    # Write 5 sell orders
    for i in range(5):
        order_data = struct.pack('<QIddBQB',
            8000000 + i,        # order_id
            0,                  # market_id (BTC)
            95010.0 + i * 10,   # price
            1.0 + i * 0.2,      # size
            0,                  # is_buy (False)
            base_time + (5 + i) * 1000,  # timestamp_ns
            0                   # status (Open)
        )
        f.write(order_data)
        print(f"Sell order {i+1}: ${95010.0 + i * 10:.2f} x {1.0 + i * 0.2:.1f}")

# Copy to the monitored directory
target = f"/var/lib/docker/volumes/hyperliquid_hl-data/_data/data/node_order_statuses/{timestamp}.bin"
os.system(f"sudo cp {temp_file} {target}")
os.system(f"sudo chmod 644 {target}")
print(f"\nCreated: {target}")

# Clean up
os.remove(temp_file)