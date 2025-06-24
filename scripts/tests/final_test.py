#!/usr/bin/env python3
import struct
import time
import os
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import threading

def create_orders_continuously():
    """Create new order files every 2 seconds"""
    for i in range(5):
        time.sleep(2)
        
        timestamp = int(time.time())
        temp_file = f"/tmp/{timestamp}.bin"
        
        with open(temp_file, "wb") as f:
            base_time = int(time.time() * 1e9)
            
            # Create orders with varying prices
            for j in range(10):
                # Buy orders
                order_data = struct.pack('<QIddBQB',
                    900000 + i * 100 + j,   # order_id
                    0,                      # market_id (BTC)
                    94500.0 + j * 10,       # price
                    1.0 + j * 0.1,          # size
                    1,                      # is_buy
                    base_time + j * 1000,   # timestamp_ns
                    0                       # status
                )
                f.write(order_data)
                
                # Sell orders
                order_data = struct.pack('<QIddBQB',
                    950000 + i * 100 + j,   # order_id
                    0,                      # market_id (BTC)
                    95500.0 + j * 10,       # price
                    1.0 + j * 0.1,          # size
                    0,                      # is_buy
                    base_time + (10+j) * 1000,  # timestamp_ns
                    0                       # status
                )
                f.write(order_data)
        
        # Copy to the directory the service is monitoring
        target = f"/home/ubuntu/node/hl/data/order_statuses/{timestamp}.bin"
        os.system(f"sudo cp {temp_file} {target}")
        os.system(f"sudo chmod 644 {target}")
        print(f"→ Created order file: {timestamp}.bin")

def test_l2_stream():
    """Test the L2 stream with live updates"""
    time.sleep(1)  # Wait for service to start
    
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Start order creation thread
    order_thread = threading.Thread(target=create_orders_continuously)
    order_thread.daemon = True
    order_thread.start()
    
    print("\nTesting L2 Real-Time Stream")
    print("="*60)
    
    # Subscribe to stream
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0],
        depth=5,
        update_interval_ms=500
    )
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        update_count = 0
        data_updates = 0
        
        for snapshot in stream:
            update_count += 1
            
            if len(snapshot.bids) > 0 or len(snapshot.asks) > 0:
                data_updates += 1
                print(f"\n✓ L2 UPDATE #{data_updates} (Total: {update_count})")
                print(f"  Sequence: {snapshot.sequence}")
                print(f"  Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
                
                if snapshot.bids:
                    print(f"  Top Bids:")
                    for i, bid in enumerate(snapshot.bids[:3]):
                        print(f"    {i+1}. ${bid.price:.2f} x {bid.quantity:.2f}")
                
                if snapshot.asks:
                    print(f"  Top Asks:")
                    for i, ask in enumerate(snapshot.asks[:3]):
                        print(f"    {i+1}. ${ask.price:.2f} x {ask.quantity:.2f}")
                
                if data_updates >= 3:
                    print("\n" + "="*60)
                    print("✓ L2 REAL-TIME STREAM IS WORKING!")
                    print("="*60)
                    return True
            
            if update_count > 30:
                print(f"\n✗ No orderbook data after {update_count} updates")
                return False
                
    except grpc.RpcError as e:
        print(f"\n✗ Stream error: {e.code()} - {e.details()}")
        return False

# Start the service
print("Starting orderbook service...")
os.system("sudo RUST_LOG=info ./target/release/orderbook-service > /dev/null 2>&1 &")

# Run the test
if test_l2_stream():
    print("\n✓ SUCCESS: L2 real-time stream is working end-to-end!")
else:
    print("\n✗ FAILED: Could not verify L2 stream")

# Cleanup
os.system("ps aux | grep orderbook-service | grep -v grep | awk '{print $2}' | xargs -r sudo kill -9")