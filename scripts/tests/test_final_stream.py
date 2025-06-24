#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import struct
import time
import os
import threading

def create_order_updates():
    """Create order updates to trigger stream updates"""
    time.sleep(2)  # Wait for stream to connect
    
    output_dir = "/home/ubuntu/node/hl/data/order_statuses"
    
    # Create a new file with timestamp to trigger file monitor
    timestamp = int(time.time())
    temp_file = f"/tmp/{timestamp}.bin"
    
    with open(temp_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        # Binary format expected by service: order_id(8), market_id(4), price(8), size(8), is_buy(1), timestamp_ns(8), status(1)
        for i in range(5):
            # Pack buy order
            data = struct.pack('<QIddBQB',
                50000 + i,          # order_id (u64)
                0,                  # market_id (u32) - BTC
                94950.0 - i * 10,   # price (f64)
                1.0 + i * 0.5,      # size (f64)
                1,                  # is_buy (u8) - True
                base_time + i * 1000000,  # timestamp_ns (u64)
                0                   # status (u8) - Open
            )
            f.write(data)
            
        for i in range(5):
            # Pack sell order
            data = struct.pack('<QIddBQB',
                60000 + i,          # order_id
                0,                  # market_id - BTC
                95050.0 + i * 10,   # price
                1.0 + i * 0.5,      # size
                0,                  # is_buy - False
                base_time + (5 + i) * 1000000,  # timestamp_ns
                0                   # status - Open
            )
            f.write(data)
    
    # Move to monitored directory with numeric name
    final_file = os.path.join(output_dir, f"{timestamp}.bin")
    os.system(f"sudo mv {temp_file} {final_file}")
    os.system(f"sudo chmod 644 {final_file}")
    print(f"→ Created new orders in {final_file}")

def test_final_stream():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    print("L2 Real-Time Stream Test")
    print("="*60)
    
    # Start order creation in background
    order_thread = threading.Thread(target=create_order_updates)
    order_thread.daemon = True
    order_thread.start()
    
    # Subscribe to stream
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0],  # BTC-PERP
        depth=10,
        update_interval_ms=100
    )
    
    print("Connecting to orderbook stream...")
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        update_count = 0
        orderbook_updates = 0
        
        for snapshot in stream:
            update_count += 1
            
            if len(snapshot.bids) > 0 or len(snapshot.asks) > 0:
                orderbook_updates += 1
                print(f"\n✓ ORDERBOOK UPDATE #{orderbook_updates}")
                print(f"  Sequence: {snapshot.sequence}")
                print(f"  Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
                
                if snapshot.bids and snapshot.asks:
                    best_bid = snapshot.bids[0]
                    best_ask = snapshot.asks[0]
                    spread = best_ask.price - best_bid.price
                    
                    print(f"  Best Bid: ${best_bid.price:,.2f} x {best_bid.quantity:.2f}")
                    print(f"  Best Ask: ${best_ask.price:,.2f} x {best_ask.quantity:.2f}")
                    print(f"  Spread: ${spread:.2f}")
                
                if orderbook_updates >= 2:
                    print("\n" + "="*60)
                    print("✓ L2 REAL-TIME STREAM WORKING!")
                    print(f"  Total updates: {update_count}")
                    print(f"  Orderbook updates: {orderbook_updates}")
                    print("="*60)
                    return True
            
            if update_count > 50:
                print(f"\n✗ Received {update_count} updates but no orderbook data")
                return False
                
    except grpc.RpcError as e:
        print(f"\n✗ Stream failed: {e.code()} - {e.details()}")
        return False

if __name__ == "__main__":
    if test_final_stream():
        print("\n✓ Test passed!")
    else:
        print("\n✗ Test failed!")