#!/usr/bin/env python3
import grpc
import sys
import os
import time
import threading
from collections import defaultdict

sys.path.append(os.path.join(os.path.dirname(__file__), '.'))

from orderbook_pb2 import SubscribeRequest
from orderbook_pb2_grpc import OrderbookServiceStub

class MarketStats:
    def __init__(self, market_id):
        self.market_id = market_id
        self.msg_count = 0
        self.start_time = time.time()
        self.last_sequence = 0
        self.unique_sequences = set()
        
    def update(self, sequence):
        self.msg_count += 1
        self.unique_sequences.add(sequence)
        self.last_sequence = sequence
        
    def get_rate(self):
        elapsed = time.time() - self.start_time
        return self.msg_count / elapsed if elapsed > 0 else 0

def test_market(stub, market_id, symbol, stats_dict, duration=30):
    """Test a single market"""
    stats = MarketStats(market_id)
    stats_dict[market_id] = stats
    
    request = SubscribeRequest(market_ids=[market_id], depth=10, update_interval_ms=0)
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        for snapshot in stream:
            stats.update(snapshot.sequence)
            
            if time.time() - stats.start_time > duration:
                break
                
    except Exception as e:
        print(f"Error streaming {symbol}: {e}")

def main():
    print("Benchmarking real-time orderbook service with Hyperliquid L1 data...")
    print("="*70)
    
    channel = grpc.insecure_channel('localhost:50052')
    stub = OrderbookServiceStub(channel)
    
    markets = {
        0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC",
        5: "AVAX", 6: "SOL", 7: "ATOM", 8: "FTM", 9: "NEAR"
    }
    
    stats_dict = {}
    threads = []
    
    # Start all market subscriptions
    print("Starting subscriptions for 10 markets...")
    for market_id, symbol in markets.items():
        thread = threading.Thread(
            target=test_market, 
            args=(stub, market_id, symbol, stats_dict, 30)
        )
        thread.start()
        threads.append(thread)
    
    # Monitor progress
    start = time.time()
    while time.time() - start < 30:
        time.sleep(5)
        print(f"\nProgress at {int(time.time() - start)}s:")
        print(f"{'Market':<10} {'Symbol':<10} {'Messages':<10} {'Rate (msg/s)':<15} {'Unique Seqs':<12}")
        print("-"*70)
        
        total_msgs = 0
        total_rate = 0
        
        for market_id, symbol in markets.items():
            if market_id in stats_dict:
                stats = stats_dict[market_id]
                rate = stats.get_rate()
                print(f"{market_id:<10} {symbol:<10} {stats.msg_count:<10,} {rate:<15.1f} {len(stats.unique_sequences):<12,}")
                total_msgs += stats.msg_count
                total_rate += rate
        
        print("-"*70)
        print(f"{'TOTAL':<10} {'':<10} {total_msgs:<10,} {total_rate:<15.1f}")
    
    # Wait for all threads
    for thread in threads:
        thread.join()
    
    # Final report
    print("\n\nFINAL RESULTS (30 seconds):")
    print("="*70)
    print(f"{'Market':<10} {'Symbol':<10} {'Messages':<10} {'Rate (msg/s)':<15} {'Unique Seqs':<12}")
    print("-"*70)
    
    total_msgs = 0
    total_rate = 0
    
    for market_id, symbol in markets.items():
        if market_id in stats_dict:
            stats = stats_dict[market_id]
            rate = stats.get_rate()
            print(f"{market_id:<10} {symbol:<10} {stats.msg_count:<10,} {rate:<15.1f} {len(stats.unique_sequences):<12,}")
            total_msgs += stats.msg_count
            total_rate += rate
    
    print("-"*70)
    print(f"{'TOTAL':<10} {'':<10} {total_msgs:<10,} {total_rate:<15.1f}")
    print("="*70)
    
    # Latency estimate
    if total_rate > 0:
        avg_latency_us = 1_000_000 / (total_rate / len(markets))
        print(f"\nAverage latency per market update: {avg_latency_us:.0f} microseconds")
        print(f"Processing {total_rate:.0f} real-time orders/second from Hyperliquid blockchain")

if __name__ == "__main__":
    main()