#!/usr/bin/env python3
"""
Example client for subscribing to orderbook streams from the Rust gRPC server.

Prerequisites:
    pip install grpcio grpcio-tools

Generate Python gRPC code:
    python -m grpc_tools.protoc -I./proto --python_out=. --grpc_python_out=. ./proto/orderbook.proto
"""

import asyncio
import grpc
import sys
from typing import List

# Import the generated protobuf modules
# You'll need to generate these first using the command above
try:
    import orderbook_pb2
    import orderbook_pb2_grpc
except ImportError:
    print("Please generate the Python gRPC code first:")
    print("python -m grpc_tools.protoc -I./proto --python_out=. --grpc_python_out=. ./proto/orderbook.proto")
    sys.exit(1)


async def subscribe_to_orderbook(server_address: str, market_ids: List[int]):
    """Subscribe to orderbook updates for specified markets."""
    
    # Create a channel
    async with grpc.aio.insecure_channel(server_address) as channel:
        # Create a stub (client)
        stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
        
        # Create subscription request
        request = orderbook_pb2.SubscribeRequest(market_ids=market_ids)
        
        print(f"Subscribing to markets: {market_ids}")
        
        try:
            # Subscribe to the stream
            async for snapshot in stub.SubscribeOrderbook(request):
                print(f"\n--- Orderbook Update ---")
                print(f"Market ID: {snapshot.market_id}")
                print(f"Symbol: {snapshot.symbol}")
                print(f"Timestamp: {snapshot.timestamp}")
                print(f"Sequence: {snapshot.sequence}")
                
                # Display top 5 levels
                print("\nBids (top 5):")
                for i, level in enumerate(snapshot.bids[:5]):
                    print(f"  {i+1}. Price: {level.price:.6f}, Size: {level.size:.6f}")
                
                print("\nAsks (top 5):")
                for i, level in enumerate(snapshot.asks[:5]):
                    print(f"  {i+1}. Price: {level.price:.6f}, Size: {level.size:.6f}")
                
                if snapshot.bids and snapshot.asks:
                    spread = snapshot.asks[0].price - snapshot.bids[0].price
                    mid = (snapshot.asks[0].price + snapshot.bids[0].price) / 2
                    print(f"\nSpread: {spread:.6f}")
                    print(f"Mid: {mid:.6f}")
                
        except grpc.RpcError as e:
            print(f"RPC failed: {e.code()}: {e.details()}")


async def get_orderbook_snapshot(server_address: str, market_id: int, depth: int = 10):
    """Get a single orderbook snapshot."""
    
    async with grpc.aio.insecure_channel(server_address) as channel:
        stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
        
        request = orderbook_pb2.GetOrderbookRequest(
            market_id=market_id,
            depth=depth
        )
        
        try:
            response = await stub.GetOrderbook(request)
            print(f"\n--- Orderbook Snapshot ---")
            print(f"Market ID: {response.market_id}")
            print(f"Symbol: {response.symbol}")
            print(f"Timestamp: {response.timestamp}")
            print(f"Sequence: {response.sequence}")
            
            print(f"\nBids (top {min(depth, len(response.bids))}):")
            for i, level in enumerate(response.bids[:depth]):
                print(f"  {i+1}. Price: {level.price:.6f}, Size: {level.size:.6f}")
            
            print(f"\nAsks (top {min(depth, len(response.asks))}):")
            for i, level in enumerate(response.asks[:depth]):
                print(f"  {i+1}. Price: {level.price:.6f}, Size: {level.size:.6f}")
                
        except grpc.RpcError as e:
            print(f"RPC failed: {e.code()}: {e.details()}")


async def main():
    # Server address
    server_address = "localhost:50051"
    
    # Market IDs: 0 = BTC, 159 = HYPE
    market_ids = [0, 159]
    
    print("Orderbook Client Example")
    print("1. Subscribe to orderbook streams")
    print("2. Get orderbook snapshot")
    print("3. Exit")
    
    choice = input("Select option: ")
    
    if choice == "1":
        # Subscribe to orderbook updates
        await subscribe_to_orderbook(server_address, market_ids)
    elif choice == "2":
        # Get snapshot
        market_id = int(input("Enter market ID (0=BTC, 159=HYPE): "))
        depth = int(input("Enter depth (default 10): ") or "10")
        await get_orderbook_snapshot(server_address, market_id, depth)
    else:
        print("Exiting...")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nInterrupted by user")