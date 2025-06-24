#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import threading
import time

def stream_thread(stub):
    """Thread to handle the stream"""
    try:
        request = orderbook_pb2.SubscribeRequest(
            market_ids=[0],
            depth=5,
            update_interval_ms=1000
        )
        
        print("Starting stream...")
        stream = stub.SubscribeOrderbook(request)
        
        for i, update in enumerate(stream):
            print(f"\n✓ Update #{i+1} received!")
            print(f"  Market: {update.symbol}")
            print(f"  Bids: {len(update.bids)}, Asks: {len(update.asks)}")
            if i >= 2:
                print("\n✓ L2 real-time stream is working!")
                break
                
    except grpc.RpcError as e:
        print(f"\n✗ Stream error: {e.code()} - {e.details()}")
    except Exception as e:
        print(f"\n✗ Unexpected error: {e}")

def main():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # First get a snapshot to verify connection
    try:
        req = orderbook_pb2.GetOrderbookRequest(market_id=0, depth=5)
        snapshot = stub.GetOrderbook(req)
        print(f"✓ Connected to orderbook service")
        print(f"  Market: {snapshot.symbol}")
        print(f"  Current state: {len(snapshot.bids)} bids, {len(snapshot.asks)} asks")
    except:
        print("✗ Failed to connect to orderbook service")
        return
    
    # Start streaming in a thread
    thread = threading.Thread(target=stream_thread, args=(stub,))
    thread.daemon = True
    thread.start()
    
    # Wait for the thread
    thread.join(timeout=10)
    
    if thread.is_alive():
        print("\n✗ Stream timed out after 10 seconds")
    
if __name__ == "__main__":
    main()