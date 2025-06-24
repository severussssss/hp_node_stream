#!/usr/bin/env python3
import grpc
import sys
import time
from concurrent import futures

# Import from the local directory since the proto is different
import orderbook_pb2
import orderbook_pb2_grpc

def test_l2_stream():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Subscribe to BTC-PERP (market_id=0) and HYPE-PERP (market_id=159)
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0, 159],
        depth=10,
        update_interval_ms=100  # 100ms updates
    )
    
    print("Subscribing to L2 orderbook stream for BTC-PERP (0) and HYPE-PERP (159)...")
    print("-" * 60)
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        update_count = 0
        for snapshot in stream:
            update_count += 1
            print(f"\nUpdate #{update_count} received at {time.strftime('%H:%M:%S')}")
            print(f"Market ID: {snapshot.market_id} ({snapshot.symbol})")
            print(f"Timestamp: {snapshot.timestamp_us}")
            print(f"Sequence: {snapshot.sequence}")
            
            if snapshot.bids:
                print(f"Bids (top 5):")
                for i, bid in enumerate(snapshot.bids[:5]):
                    print(f"  {i+1}. Price: {bid.price:.2f}, Qty: {bid.quantity:.4f}, Orders: {bid.order_count}")
            
            if snapshot.asks:
                print(f"Asks (top 5):")
                for i, ask in enumerate(snapshot.asks[:5]):
                    print(f"  {i+1}. Price: {ask.price:.2f}, Qty: {ask.quantity:.4f}, Orders: {ask.order_count}")
            
            if snapshot.bids and snapshot.asks:
                spread = snapshot.asks[0].price - snapshot.bids[0].price
                mid = (snapshot.asks[0].price + snapshot.bids[0].price) / 2
                print(f"Spread: {spread:.2f}, Mid: {mid:.2f}")
            
            print("-" * 60)
            
            if update_count >= 10:
                print("\nReceived 10 updates successfully. Test passed!")
                break
                
    except grpc.RpcError as e:
        print(f"RPC Error: {e.code()} - {e.details()}")
        return False
    except KeyboardInterrupt:
        print("\nTest interrupted by user")
        return True
    
    return True

if __name__ == "__main__":
    test_l2_stream()