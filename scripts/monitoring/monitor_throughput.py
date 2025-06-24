#!/usr/bin/env python3
import grpc
import time
import sys
from collections import defaultdict
from orderbook_pb2 import SubscribeRequest
from orderbook_pb2_grpc import OrderbookServiceStub

def monitor_throughput(duration=30):
    channel = grpc.insecure_channel('localhost:50052')
    stub = OrderbookServiceStub(channel)
    
    # Subscribe to first 5 markets
    request = SubscribeRequest(
        market_ids=[0, 1, 2, 3, 4],  # BTC, ETH, ARB, OP, MATIC
        depth=10,
        update_interval_ms=0  # Real-time
    )
    
    updates_per_market = defaultdict(int)
    sequences_per_market = defaultdict(list)
    start_time = time.time()
    last_print = start_time
    
    print(f"Monitoring real-time updates for {duration} seconds...")
    print("Markets: BTC(0), ETH(1), ARB(2), OP(3), MATIC(4)\n")
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        for snapshot in stream:
            updates_per_market[snapshot.market_id] += 1
            sequences_per_market[snapshot.market_id].append(snapshot.sequence)
            
            # Print stats every 5 seconds
            current_time = time.time()
            if current_time - last_print >= 5:
                elapsed = current_time - start_time
                print(f"\n[{elapsed:.0f}s] Update counts:")
                
                total_updates = 0
                for market_id in sorted(updates_per_market.keys()):
                    count = updates_per_market[market_id]
                    rate = count / elapsed
                    total_updates += count
                    
                    # Check sequence gaps
                    seqs = sequences_per_market[market_id]
                    if len(seqs) > 1:
                        gaps = sum(1 for i in range(1, len(seqs)) if seqs[i] != seqs[i-1] + 1)
                    else:
                        gaps = 0
                    
                    symbol = {0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC"}.get(market_id, f"ID{market_id}")
                    print(f"  {symbol}: {count} updates ({rate:.1f}/sec), gaps: {gaps}")
                
                print(f"  Total: {total_updates} updates ({total_updates/elapsed:.1f}/sec)")
                last_print = current_time
            
            if current_time - start_time >= duration:
                break
                
    except grpc.RpcError as e:
        print(f"RPC Error: {e}")
    except KeyboardInterrupt:
        print("\nInterrupted by user")
    
    # Final summary
    elapsed = time.time() - start_time
    print(f"\n{'='*50}")
    print(f"FINAL SUMMARY (ran for {elapsed:.1f} seconds):")
    print(f"{'='*50}")
    
    total_updates = 0
    for market_id in sorted(updates_per_market.keys()):
        count = updates_per_market[market_id]
        rate = count / elapsed
        total_updates += count
        
        symbol = {0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC"}.get(market_id, f"ID{market_id}")
        print(f"{symbol}: {count} updates ({rate:.2f} updates/sec)")
    
    print(f"\nTotal updates: {total_updates} ({total_updates/elapsed:.2f} updates/sec)")
    
    # Check if updates are real-time by looking at sequence numbers
    print("\nReal-time verification (checking sequence continuity):")
    for market_id, seqs in sequences_per_market.items():
        if len(seqs) > 1:
            gaps = sum(1 for i in range(1, len(seqs)) if seqs[i] != seqs[i-1] + 1)
            symbol = {0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC"}.get(market_id, f"ID{market_id}")
            if gaps == 0:
                print(f"  {symbol}: ✓ No gaps - real-time stream confirmed")
            else:
                print(f"  {symbol}: ⚠ {gaps} sequence gaps detected")

if __name__ == "__main__":
    monitor_throughput(30)