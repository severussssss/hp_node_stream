#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import subprocess
import time
import threading

def inject_orders():
    """Inject orders while streaming"""
    time.sleep(2)  # Wait for stream to start
    print("\n→ Injecting live orders...")
    subprocess.run(["python3", "inject_live_orders.py"])

def test_live_stream():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Start order injection in background
    inject_thread = threading.Thread(target=inject_orders)
    inject_thread.daemon = True
    inject_thread.start()
    
    # Subscribe to stream
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0],
        depth=10,
        update_interval_ms=100  # 100ms updates
    )
    
    print("Starting L2 real-time stream test...")
    print("Waiting for orderbook updates...")
    print("-" * 60)
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        update_count = 0
        updates_with_data = 0
        
        for snapshot in stream:
            update_count += 1
            
            if len(snapshot.bids) > 0 or len(snapshot.asks) > 0:
                updates_with_data += 1
                print(f"\n✓ Update #{update_count} - ORDERBOOK DATA RECEIVED!")
                print(f"  Market: {snapshot.symbol}")
                print(f"  Sequence: {snapshot.sequence}")
                print(f"  Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
                
                if snapshot.bids:
                    best_bid = snapshot.bids[0]
                    print(f"  Best Bid: ${best_bid.price:.2f} x {best_bid.quantity:.4f}")
                
                if snapshot.asks:
                    best_ask = snapshot.asks[0]
                    print(f"  Best Ask: ${best_ask.price:.2f} x {best_ask.quantity:.4f}")
                
                if snapshot.bids and snapshot.asks:
                    spread = snapshot.asks[0].price - snapshot.bids[0].price
                    print(f"  Spread: ${spread:.2f}")
                
                if updates_with_data >= 3:
                    print("\n" + "="*60)
                    print("✓ L2 REAL-TIME STREAM IS WORKING!")
                    print(f"  Received {update_count} total updates")
                    print(f"  {updates_with_data} updates had orderbook data")
                    print("="*60)
                    break
            else:
                if update_count % 10 == 0:
                    print(f"  ... received {update_count} updates (orderbook still empty)")
            
            if update_count > 100:
                print("\n✗ Received 100 updates but no orderbook data")
                break
                
    except grpc.RpcError as e:
        print(f"\n✗ RPC Error: {e.code()} - {e.details()}")
        return False
    
    return True

if __name__ == "__main__":
    test_live_stream()