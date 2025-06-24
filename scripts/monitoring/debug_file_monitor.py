#!/usr/bin/env python3
import struct
import time
import os

# Create a test file in temp first
temp_file = "/tmp/test_market_0.bin"

with open(temp_file, "wb") as f:
    # Write a single order to test
    # Format: order_id(8), market_id(4), price(8), size(8), is_buy(1), timestamp_ns(8), status(1) = 38 bytes
    order_data = struct.pack('<QIddBQB',
        99999,              # order_id
        0,                  # market_id (BTC)
        95000.0,            # price
        1.0,                # size
        1,                  # is_buy (True)
        int(time.time() * 1e9),  # timestamp_ns
        0                   # status (Open)
    )
    f.write(order_data)
    print(f"Created test order: 38 bytes")

# Try different file names to see which one works
test_names = ["0", "0.bin", str(int(time.time()))]

for name in test_names:
    target = f"/home/ubuntu/node/hl/data/order_statuses/{name}"
    os.system(f"sudo cp {temp_file} {target}")
    print(f"Created: {target}")
    time.sleep(2)
    
    # Check orderbook
    os.system("""python3 -c "
import grpc
import orderbook_pb2
import orderbook_pb2_grpc

channel = grpc.insecure_channel('localhost:50051')
stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
req = orderbook_pb2.GetOrderbookRequest(market_id=0, depth=5)
snapshot = stub.GetOrderbook(req)
print(f'  Result: Bids={len(snapshot.bids)}, Asks={len(snapshot.asks)}, Seq={snapshot.sequence}')
"
""")
    
    # Clean up
    os.system(f"sudo rm -f {target}")