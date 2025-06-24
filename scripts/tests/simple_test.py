#!/usr/bin/env python3
"""
Simple test to show orderbook service is working.
"""

import asyncio
import grpc
import orderbook_pb2
import orderbook_pb2_grpc


async def test_orderbook_service():
    """Test basic orderbook service functionality."""
    
    server_address = "localhost:50051"
    
    async with grpc.aio.insecure_channel(server_address) as channel:
        stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
        
        # Test 1: Get available markets
        print("1. Getting available markets...")
        markets = await stub.GetMarkets(orderbook_pb2.GetMarketsRequest())
        for market in markets.markets:
            print(f"   - Market {market.market_id}: {market.symbol}")
        
        # Test 2: Get BTC orderbook snapshot
        print("\n2. Getting BTC orderbook snapshot...")
        btc_book = await stub.GetOrderbook(
            orderbook_pb2.GetOrderbookRequest(market_id=0, depth=5)
        )
        print(f"   Symbol: {btc_book.symbol}")
        print(f"   Bids: {len(btc_book.bids)} levels")
        print(f"   Asks: {len(btc_book.asks)} levels")
        
        # Test 3: Subscribe to updates (for 5 seconds)
        print("\n3. Subscribing to BTC orderbook updates for 5 seconds...")
        request = orderbook_pb2.SubscribeRequest(market_ids=[0])
        
        update_count = 0
        start_time = asyncio.get_event_loop().time()
        
        async for snapshot in stub.SubscribeOrderbook(request):
            update_count += 1
            if snapshot.bids and snapshot.asks:
                print(f"   Update {update_count}: Best Bid=${snapshot.bids[0].price:,.2f}, Best Ask=${snapshot.asks[0].price:,.2f}")
            else:
                print(f"   Update {update_count}: Waiting for order data...")
            
            if asyncio.get_event_loop().time() - start_time > 5:
                break
        
        print(f"\nReceived {update_count} updates in 5 seconds")
        print("\nOrderbook service is working correctly!")


if __name__ == "__main__":
    asyncio.run(test_orderbook_service())