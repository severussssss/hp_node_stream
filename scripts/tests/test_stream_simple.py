#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import time

def test_stream():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Subscribe to just BTC-PERP with longer interval
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0],
        depth=5,
        update_interval_ms=1000  # 1 second updates
    )
    
    print("Starting L2 orderbook stream test for BTC-PERP...")
    print("Will wait for updates (empty orderbooks are expected if no orders)")
    print("-" * 60)
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        update_count = 0
        start_time = time.time()
        
        for snapshot in stream:
            update_count += 1
            elapsed = time.time() - start_time
            
            print(f"\nUpdate #{update_count} at {elapsed:.1f}s")
            print(f"Market: {snapshot.symbol} (ID: {snapshot.market_id})")
            print(f"Sequence: {snapshot.sequence}")
            print(f"Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
            
            if update_count >= 5:
                print("\n✓ Successfully received 5 updates!")
                print("L2 real-time stream is working correctly.")
                break
                
    except grpc.RpcError as e:
        print(f"RPC Error: {e.code()} - {e.details()}")
        return False
    except KeyboardInterrupt:
        print("\nTest interrupted")
    
    return True

if __name__ == "__main__":
    test_stream()