#!/usr/bin/env python3
import grpc
import sys
import os
import time
import threading
from collections import defaultdict
from concurrent.futures import ThreadPoolExecutor

sys.path.append(os.path.join(os.path.dirname(__file__), '.'))

from orderbook_pb2 import SubscribeRequest, GetMarketsRequest
from orderbook_pb2_grpc import OrderbookServiceStub

class MarketBenchmark:
    def __init__(self, market_id, symbol):
        self.market_id = market_id
        self.symbol = symbol
        self.msg_count = 0
        self.start_time = None
        self.last_sequence = 0
        self.lock = threading.Lock()
        
    def update(self, sequence):
        with self.lock:
            self.msg_count += 1
            if sequence != self.last_sequence:
                self.last_sequence = sequence
    
    def get_stats(self):
        with self.lock:
            if self.start_time is None:
                return 0, 0
            elapsed = time.time() - self.start_time
            rate = self.msg_count / elapsed if elapsed > 0 else 0
            return self.msg_count, rate

def benchmark_market(stub, market_id, symbol, duration=10):
    """Benchmark a single market"""
    benchmark = MarketBenchmark(market_id, symbol)
    
    try:
        # Subscribe to the market
        request = SubscribeRequest(market_ids=[market_id], depth=10, update_interval_ms=0)
        stream = stub.SubscribeOrderbook(request)
        
        benchmark.start_time = time.time()
        
        # Stream updates
        for snapshot in stream:
            benchmark.update(snapshot.sequence)
            
            # Check if we've run long enough
            if time.time() - benchmark.start_time > duration:
                break
                
    except grpc.RpcError as e:
        print(f"  Error for {symbol}: {e.code()} - {e.details()}")
        return 0, 0
    except Exception as e:
        print(f"  Error for {symbol}: {e}")
        return 0, 0
    
    return benchmark.get_stats()

def run_benchmarks(port=50052):
    """Run benchmarks for all markets"""
    print(f"Connecting to orderbook service at localhost:{port}...")
    
    channel = grpc.insecure_channel(f'localhost:{port}')
    stub = OrderbookServiceStub(channel)
    
    # Test connection
    try:
        markets_response = stub.GetMarkets(GetMarketsRequest())
        available_markets = {m.market_id: m.symbol for m in markets_response.markets}
        print(f"Found {len(available_markets)} markets available")
    except Exception as e:
        print(f"Failed to connect: {e}")
        return
    
    markets = {
        0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC",
        5: "AVAX", 6: "SOL", 7: "ATOM", 8: "FTM", 9: "NEAR"
    }
    
    print("\nStarting benchmarks (10 seconds per market)...")
    print("-" * 60)
    print(f"{'Market':<10} {'Symbol':<10} {'Messages':<12} {'Rate (msg/s)':<15}")
    print("-" * 60)
    
    # Run benchmarks in parallel
    with ThreadPoolExecutor(max_workers=10) as executor:
        futures = {}
        
        for market_id, symbol in markets.items():
            future = executor.submit(benchmark_market, stub, market_id, symbol, 10)
            futures[future] = (market_id, symbol)
        
        # Collect results
        results = {}
        for future in futures:
            market_id, symbol = futures[future]
            try:
                msg_count, rate = future.result(timeout=15)
                results[market_id] = (symbol, msg_count, rate)
            except Exception as e:
                print(f"Benchmark failed for {symbol}: {e}")
                results[market_id] = (symbol, 0, 0)
    
    # Print results
    total_messages = 0
    total_rate = 0
    
    for market_id in sorted(results.keys()):
        symbol, msg_count, rate = results[market_id]
        print(f"{market_id:<10} {symbol:<10} {msg_count:<12,} {rate:<15,.2f}")
        total_messages += msg_count
        total_rate += rate
    
    print("-" * 60)
    print(f"{'TOTAL':<10} {'':<10} {total_messages:<12,} {total_rate:<15,.2f}")
    print("-" * 60)
    
    # Calculate latency estimate
    if total_rate > 0:
        avg_latency_ms = 1000 / (total_rate / len(markets))
        print(f"\nAverage latency estimate: {avg_latency_ms:.2f} ms per update")

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--port', type=int, default=50052, help='gRPC port')
    args = parser.parse_args()
    
    run_benchmarks(args.port)