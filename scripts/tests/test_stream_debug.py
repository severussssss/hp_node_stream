#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import time
import logging

# Enable gRPC debug logging
logging.basicConfig(level=logging.DEBUG)

def test_stream_debug():
    # Create channel with options for debugging
    options = [
        ('grpc.keepalive_time_ms', 10000),
        ('grpc.keepalive_timeout_ms', 5000),
        ('grpc.keepalive_permit_without_calls', True),
        ('grpc.http2.max_pings_without_data', 0),
    ]
    
    channel = grpc.insecure_channel('localhost:50051', options=options)
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Simple request for just one market
    request = orderbook_pb2.SubscribeRequest(
        market_ids=[0],
        depth=10,
        update_interval_ms=5000  # 5 seconds
    )
    
    print("Testing L2 stream with debug logging...")
    print("-" * 60)
    
    try:
        # Create the stream
        stream = stub.SubscribeOrderbook(request)
        
        print("Stream created, waiting for data...")
        
        # Try to get just one update
        for update in stream:
            print(f"Received update!")
            print(f"Market: {update.symbol} (ID: {update.market_id})")
            print(f"Timestamp: {update.timestamp_us}")
            print(f"Bids: {len(update.bids)}, Asks: {len(update.asks)}")
            break
            
        print("Successfully received an update!")
        
    except grpc.RpcError as e:
        print(f"\nRPC Error Details:")
        print(f"Code: {e.code()}")
        print(f"Details: {e.details()}")
        print(f"Debug: {e.debug_error_string()}")
    except Exception as e:
        print(f"\nUnexpected error: {type(e).__name__}: {e}")

if __name__ == "__main__":
    test_stream_debug()