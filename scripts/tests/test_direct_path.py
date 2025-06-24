#!/usr/bin/env python3
import struct
import time
import os

# Create directly in the real path, not through symlink
real_path = "/var/lib/docker/volumes/hyperliquid_hl-data/_data/data/node_order_statuses"

# Create a simple test file
timestamp = int(time.time())
temp_file = f"/tmp/{timestamp}.bin"

with open(temp_file, "wb") as f:
    # Single buy order for BTC
    order_data = struct.pack('<QIddBQB',
        888888,             # order_id
        0,                  # market_id (BTC)
        94999.0,            # price
        5.0,                # size
        1,                  # is_buy (True)
        int(time.time() * 1e9),  # timestamp_ns
        0                   # status (Open)
    )
    f.write(order_data)
    
    # Single sell order
    order_data = struct.pack('<QIddBQB',
        888889,             # order_id
        0,                  # market_id (BTC)
        95001.0,            # price
        5.0,                # size
        0,                  # is_buy (False)
        int(time.time() * 1e9),  # timestamp_ns
        0                   # status (Open)
    )
    f.write(order_data)

# Copy to real directory
target = f"{real_path}/{timestamp}.bin"
os.system(f"sudo cp {temp_file} {target}")
os.system(f"sudo chmod 644 {target}")
print(f"Created: {target}")

# Wait and check
import grpc
import orderbook_pb2
import orderbook_pb2_grpc

time.sleep(3)

channel = grpc.insecure_channel('localhost:50051')
stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
req = orderbook_pb2.GetOrderbookRequest(market_id=0, depth=5)
snapshot = stub.GetOrderbook(req)

print(f"Result: Bids={len(snapshot.bids)}, Asks={len(snapshot.asks)}")
if snapshot.bids and snapshot.asks:
    print(f"Best Bid: ${snapshot.bids[0].price:.2f}")
    print(f"Best Ask: ${snapshot.asks[0].price:.2f}")
    print("✓ Orders processed!")