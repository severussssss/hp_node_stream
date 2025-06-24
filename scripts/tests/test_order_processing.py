#!/usr/bin/env python3
import struct
import time
import os
import grpc
import orderbook_pb2
import orderbook_pb2_grpc

def create_order_file():
    """Create a properly formatted order file"""
    temp_file = "/tmp/999999.bin"  # Numeric name with .bin extension
    
    with open(temp_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        # Create 10 buy orders
        for i in range(10):
            order_data = struct.pack('<QIddBQB',
                100000 + i,         # order_id
                0,                  # market_id (BTC)
                94000.0 + i * 100,  # price (94000-94900)
                0.1 * (i + 1),      # size (0.1-1.0)
                1,                  # is_buy (True)
                base_time + i * 1000000,  # timestamp_ns
                0                   # status (Open)
            )
            f.write(order_data)
        
        # Create 10 sell orders
        for i in range(10):
            order_data = struct.pack('<QIddBQB',
                200000 + i,         # order_id
                0,                  # market_id (BTC)
                95000.0 + i * 100,  # price (95000-95900)
                0.1 * (i + 1),      # size (0.1-1.0)
                0,                  # is_buy (False)
                base_time + (10 + i) * 1000000,  # timestamp_ns
                0                   # status (Open)
            )
            f.write(order_data)
    
    # Move to monitored directory
    target = "/home/ubuntu/node/hl/data/order_statuses/999999.bin"
    os.system(f"sudo mv {temp_file} {target}")
    os.system(f"sudo chmod 644 {target}")
    print(f"Created order file: {target}")
    return target

def check_orderbook():
    """Check orderbook state"""
    try:
        channel = grpc.insecure_channel('localhost:50051')
        stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
        req = orderbook_pb2.GetOrderbookRequest(market_id=0, depth=5)
        snapshot = stub.GetOrderbook(req)
        
        print(f"Orderbook state:")
        print(f"  Sequence: {snapshot.sequence}")
        print(f"  Bids: {len(snapshot.bids)}")
        print(f"  Asks: {len(snapshot.asks)}")
        
        if snapshot.bids:
            print(f"  Best bid: ${snapshot.bids[0].price:.2f}")
        if snapshot.asks:
            print(f"  Best ask: ${snapshot.asks[0].price:.2f}")
            
        return len(snapshot.bids) > 0 or len(snapshot.asks) > 0
    except Exception as e:
        print(f"Error checking orderbook: {e}")
        return False

def main():
    print("Testing order file processing...")
    print("-" * 50)
    
    # Check initial state
    print("Initial orderbook:")
    check_orderbook()
    
    # Create order file
    print("\nCreating order file...")
    order_file = create_order_file()
    
    # Wait for processing
    print("\nWaiting for file to be processed...")
    for i in range(10):
        time.sleep(1)
        print(f"\nAttempt {i+1}/10:")
        if check_orderbook():
            print("\n✓ Orders processed successfully!")
            break
    else:
        print("\n✗ Orders were not processed")
        
        # Check if file still exists
        if os.path.exists(order_file):
            print(f"File still exists: {order_file}")
            os.system(f"sudo ls -la {order_file}")
        else:
            print(f"File was consumed: {order_file}")

if __name__ == "__main__":
    main()