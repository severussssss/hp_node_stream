#!/usr/bin/env python3
import grpc
import sys
import os

sys.path.append(os.path.join(os.path.dirname(__file__), '.'))

from orderbook_pb2 import SubscribeRequest, GetOrderbookRequest, GetMarketsRequest
from orderbook_pb2_grpc import OrderbookServiceStub

def test_connection():
    """Test basic connectivity"""
    print("Testing orderbook service connection...")
    
    channel = grpc.insecure_channel('localhost:50052')
    stub = OrderbookServiceStub(channel)
    
    # Test 1: Get markets
    print("\n1. Testing GetMarkets...")
    try:
        response = stub.GetMarkets(GetMarketsRequest())
        print(f"   Success! Found {len(response.markets)} markets:")
        for m in response.markets:
            print(f"   - Market {m.market_id}: {m.symbol}")
    except Exception as e:
        print(f"   Failed: {e}")
    
    # Test 2: Get single orderbook snapshot
    print("\n2. Testing GetOrderbook for BTC (market 0)...")
    try:
        response = stub.GetOrderbook(GetOrderbookRequest(market_id=0, depth=5))
        print(f"   Success! Sequence: {response.sequence}")
        print(f"   Bids: {len(response.bids)}, Asks: {len(response.asks)}")
        if response.bids:
            print(f"   Best bid: ${response.bids[0].price:.2f}")
        if response.asks:
            print(f"   Best ask: ${response.asks[0].price:.2f}")
    except Exception as e:
        print(f"   Failed: {e}")
    
    # Test 3: Stream test (just get first message)
    print("\n3. Testing streaming subscription...")
    try:
        request = SubscribeRequest(market_ids=[0], depth=5, update_interval_ms=0)
        stream = stub.SubscribeOrderbook(request)
        
        # Just get first message
        snapshot = next(iter(stream))
        print(f"   Success! Got snapshot with sequence {snapshot.sequence}")
        print(f"   Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
        
    except Exception as e:
        print(f"   Failed: {e}")

if __name__ == "__main__":
    test_connection()