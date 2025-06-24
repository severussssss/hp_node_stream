#!/usr/bin/env python3
import grpc
import sys
import os
import time
import threading
from collections import defaultdict

sys.path.append(os.path.join(os.path.dirname(__file__), '.'))

from orderbook_pb2 import SubscribeRequest, GetMarketsRequest
from orderbook_pb2_grpc import OrderbookServiceStub

def test_single_market(port=50052, market_id=0, duration=5):
    """Quick test of a single market"""
    channel = grpc.insecure_channel(f'localhost:{port}')
    stub = OrderbookServiceStub(channel)
    
    request = SubscribeRequest(market_ids=[market_id], depth=10, update_interval_ms=0)
    
    print(f"Testing market {market_id} for {duration} seconds...")
    
    count = 0
    start_time = time.time()
    last_log = start_time
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        for snapshot in stream:
            count += 1
            
            # Log every second
            now = time.time()
            if now - last_log >= 1.0:
                elapsed = now - start_time
                rate = count / elapsed
                print(f"  Market {market_id}: {count} messages, {rate:.2f} msg/s")
                last_log = now
            
            if now - start_time > duration:
                break
                
    except Exception as e:
        print(f"Error: {e}")
        return 0
    
    elapsed = time.time() - start_time
    final_rate = count / elapsed if elapsed > 0 else 0
    return count, final_rate

def main():
    print("Quick benchmark of orderbook service...")
    print("-" * 50)
    
    # Test each market for 5 seconds
    markets = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
    
    total_messages = 0
    total_rate = 0
    
    for market_id in markets:
        count, rate = test_single_market(market_id=market_id, duration=5)
        print(f"Market {market_id} final: {count} messages, {rate:.2f} msg/s")
        print("-" * 50)
        total_messages += count
        total_rate += rate
    
    print(f"\nSummary:")
    print(f"Total messages: {total_messages}")
    print(f"Average rate per market: {total_rate/len(markets):.2f} msg/s")
    print(f"Total throughput: {total_rate:.2f} msg/s")
    
    if total_rate > 0:
        avg_latency_us = 1_000_000 / (total_rate / len(markets))
        print(f"Average latency estimate: {avg_latency_us:.0f} microseconds")

if __name__ == "__main__":
    main()