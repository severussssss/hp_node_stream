#!/usr/bin/env python3
"""
Test script to demonstrate BTC perpetual L2 orderbook streaming.
"""

import asyncio
import grpc
import sys
from datetime import datetime

# Import the generated protobuf modules
import orderbook_pb2
import orderbook_pb2_grpc


async def stream_btc_orderbook():
    """Subscribe to BTC perp orderbook updates."""
    
    # Connect to local gRPC server
    server_address = "localhost:50051"
    
    print(f"Connecting to orderbook service at {server_address}...")
    
    async with grpc.aio.insecure_channel(server_address) as channel:
        # Create a stub (client)
        stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
        
        # First, get available markets
        try:
            markets_response = await stub.GetMarkets(orderbook_pb2.GetMarketsRequest())
            print("\nAvailable markets:")
            for market in markets_response.markets:
                print(f"  Market ID: {market.market_id}, Symbol: {market.symbol}, Active: {market.active}")
        except grpc.RpcError as e:
            print(f"Failed to get markets: {e}")
        
        # Subscribe to BTC orderbook (market ID 0)
        request = orderbook_pb2.SubscribeRequest(
            market_ids=[0],  # BTC perpetual
            depth=10  # Top 10 levels each side
        )
        
        print(f"\nSubscribing to BTC perpetual orderbook (Market ID: 0)...")
        print("=" * 80)
        
        try:
            # Subscribe to the stream
            update_count = 0
            async for snapshot in stub.SubscribeOrderbook(request):
                update_count += 1
                
                # Print header
                timestamp = datetime.fromtimestamp(snapshot.timestamp_us / 1_000_000)
                print(f"\n[Update #{update_count}] {timestamp.strftime('%Y-%m-%d %H:%M:%S.%f')[:-3]}")
                print(f"Market: {snapshot.symbol} (ID: {snapshot.market_id})")
                print(f"Sequence: {snapshot.sequence}")
                print("-" * 40)
                
                # Display orderbook
                if snapshot.bids or snapshot.asks:
                    # Calculate spread and mid price
                    if snapshot.bids and snapshot.asks:
                        best_bid = snapshot.bids[0].price
                        best_ask = snapshot.asks[0].price
                        spread = best_ask - best_bid
                        mid_price = (best_bid + best_ask) / 2
                        
                        print(f"Mid Price: ${mid_price:,.2f}")
                        print(f"Spread: ${spread:.2f} ({(spread/mid_price)*100:.3f}%)")
                        print()
                    
                    # Display asks (reversed for visual clarity)
                    print("ASKS (Sell Orders):")
                    for i, level in enumerate(reversed(snapshot.asks[:5])):
                        print(f"  ${level.price:>10,.2f} | {level.quantity:>10,.4f}")
                    
                    print("  " + "-" * 35)
                    
                    # Display bids
                    print("BIDS (Buy Orders):")
                    for i, level in enumerate(snapshot.bids[:5]):
                        print(f"  ${level.price:>10,.2f} | {level.quantity:>10,.4f}")
                    
                    # Calculate total liquidity in top 5 levels
                    bid_liquidity = sum(level.quantity * level.price for level in snapshot.bids[:5])
                    ask_liquidity = sum(level.quantity * level.price for level in snapshot.asks[:5])
                    print()
                    print(f"Top 5 Bid Liquidity: ${bid_liquidity:,.2f}")
                    print(f"Top 5 Ask Liquidity: ${ask_liquidity:,.2f}")
                else:
                    print("Waiting for orderbook data...")
                
                print("=" * 80)
                
        except grpc.RpcError as e:
            print(f"\nRPC failed: {e.code()}: {e.details()}")
        except KeyboardInterrupt:
            print("\n\nStream interrupted by user")


async def main():
    print("BTC Perpetual L2 Orderbook Stream Demo")
    print("======================================")
    
    await stream_btc_orderbook()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        print("\nShutting down...")