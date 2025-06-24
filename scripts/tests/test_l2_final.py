#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import time
import struct
import os

def create_fresh_orders():
    """Create new orders with current timestamp"""
    timestamp = int(time.time())
    temp_file = f"/tmp/fresh_{timestamp}.bin"
    
    with open(temp_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        # Create 5 buy orders
        for i in range(5):
            order_data = struct.pack('<QIddBQB',
                9000000 + timestamp + i,  # Unique order_id
                0,                        # market_id (BTC)
                95000.0 - i * 20,         # price
                2.0,                      # size
                1,                        # is_buy
                base_time + i * 1000,     # timestamp_ns
                0                         # status (Open)
            )
            f.write(order_data)
        
        # Create 5 sell orders
        for i in range(5):
            order_data = struct.pack('<QIddBQB',
                9500000 + timestamp + i,  # Unique order_id
                0,                        # market_id (BTC)
                95100.0 + i * 20,         # price
                2.0,                      # size
                0,                        # is_buy (False)
                base_time + (5 + i) * 1000,  # timestamp_ns
                0                         # status (Open)
            )
            f.write(order_data)
    
    # Copy to monitored directory
    target = f"/var/lib/docker/volumes/hyperliquid_hl-data/_data/data/node_order_statuses/{timestamp}.bin"
    os.system(f"sudo cp {temp_file} {target}")
    os.system(f"sudo chmod 644 {target}")
    print(f"Created fresh orders: {timestamp}.bin")
    os.remove(temp_file)

def test_l2_stream_final():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    print("L2 Real-Time Stream Final Test")
    print("="*60)
    
    # Create fresh orders
    create_fresh_orders()
    time.sleep(2)
    
    # Subscribe to stream
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0],
        depth=10,
        update_interval_ms=100
    )
    
    print("\nSubscribing to L2 stream...")
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        update_count = 0
        found_data = False
        
        for snapshot in stream:
            update_count += 1
            
            if len(snapshot.bids) > 0 or len(snapshot.asks) > 0:
                found_data = True
                print(f"\n✓ L2 STREAM UPDATE WITH DATA!")
                print(f"  Update #{update_count}")
                print(f"  Market: {snapshot.symbol}")
                print(f"  Sequence: {snapshot.sequence}")
                print(f"  Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
                
                if snapshot.bids:
                    print("\n  Top Bids:")
                    for i, bid in enumerate(snapshot.bids[:3]):
                        print(f"    {i+1}. ${bid.price:,.2f} x {bid.quantity:.2f}")
                
                if snapshot.asks:
                    print("\n  Top Asks:")
                    for i, ask in enumerate(snapshot.asks[:3]):
                        print(f"    {i+1}. ${ask.price:,.2f} x {ask.quantity:.2f}")
                
                if snapshot.bids and snapshot.asks:
                    spread = snapshot.asks[0].price - snapshot.bids[0].price
                    mid = (snapshot.asks[0].price + snapshot.bids[0].price) / 2
                    print(f"\n  Spread: ${spread:.2f}")
                    print(f"  Mid Price: ${mid:,.2f}")
                
                print("\n" + "="*60)
                print("✓ L2 REAL-TIME STREAM IS WORKING WITH LIVE DATA!")
                print("="*60)
                return True
            else:
                if update_count % 10 == 0:
                    print(f"  ... received {update_count} updates (still no data)")
                    # Try creating more orders
                    if update_count == 20:
                        print("\n  Creating more orders...")
                        create_fresh_orders()
            
            if update_count > 50:
                print(f"\n✗ No data after {update_count} updates")
                return False
                
    except grpc.RpcError as e:
        print(f"\n✗ Stream error: {e.code()} - {e.details()}")
        return False

if __name__ == "__main__":
    if test_l2_stream_final():
        print("\n✓ SUCCESS!")
    else:
        print("\n✗ FAILED!")