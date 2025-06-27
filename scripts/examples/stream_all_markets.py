#!/usr/bin/env python3
"""
Example script showing how to stream all available markets using the 
auto-generated market configuration.
"""

import sys
import os
sys.path.append(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

import grpc
import time
from market_config import get_all_market_ids, get_symbol, MARKET_IDS

# Add proto path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../proto'))
import orderbook_pb2
import orderbook_pb2_grpc

def stream_all_markets(host='localhost', port=50052, max_markets=None):
    """Stream orderbook updates for all active markets"""
    channel = grpc.insecure_channel(f'{host}:{port}')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Get all market IDs
    all_market_ids = get_all_market_ids()
    
    if max_markets:
        market_ids = all_market_ids[:max_markets]
        print(f"Streaming {len(market_ids)} markets (limited to {max_markets})")
    else:
        market_ids = all_market_ids
        print(f"Streaming ALL {len(market_ids)} active markets")
    
    # Show which markets we're streaming
    print("\nMarkets to stream:")
    for i, market_id in enumerate(market_ids):
        symbol = get_symbol(market_id)
        print(f"  {market_id:3d}: {symbol}")
        if i >= 20:  # Just show first 20
            print(f"  ... and {len(market_ids) - 21} more")
            break
    
    # Subscribe to all markets
    request = orderbook_pb2.SubscribeRequest(market_ids=market_ids)
    
    print(f"\nConnecting to {host}:{port}...")
    updates_count = {}
    start_time = time.time()
    
    try:
        stream = stub.SubscribeOrderbook(request)
        
        for snapshot in stream:
            market_id = snapshot.market_id
            symbol = snapshot.symbol
            
            # Count updates per market
            if market_id not in updates_count:
                updates_count[market_id] = 0
            updates_count[market_id] += 1
            
            # Show periodic statistics
            total_updates = sum(updates_count.values())
            if total_updates % 1000 == 0:
                elapsed = time.time() - start_time
                rate = total_updates / elapsed
                
                print(f"\n[{elapsed:.1f}s] Total updates: {total_updates}, Rate: {rate:.1f}/sec")
                print("Top 10 most active markets:")
                sorted_markets = sorted(updates_count.items(), key=lambda x: x[1], reverse=True)[:10]
                for mid, count in sorted_markets:
                    sym = get_symbol(mid)
                    print(f"  {sym:8s} (ID:{mid:3d}): {count:6d} updates")
            
            # Optionally show orderbook snapshot for specific market
            if symbol == "BTC" and updates_count[market_id] % 100 == 0:
                if snapshot.bids and snapshot.asks:
                    best_bid = snapshot.bids[0].price
                    best_ask = snapshot.asks[0].price
                    spread = best_ask - best_bid
                    print(f"\nBTC: Bid=${best_bid:,.2f} Ask=${best_ask:,.2f} Spread=${spread:.2f}")
                    
    except grpc.RpcError as e:
        print(f"RPC Error: {e.code()} - {e.details()}")
    except KeyboardInterrupt:
        print("\nStopped by user")
    finally:
        channel.close()
        
        # Final statistics
        elapsed = time.time() - start_time
        total_updates = sum(updates_count.values())
        
        print(f"\nFinal Statistics ({elapsed:.1f} seconds):")
        print(f"Total updates: {total_updates}")
        print(f"Average rate: {total_updates/elapsed:.1f} updates/sec")
        print(f"Markets with updates: {len(updates_count)}/{len(market_ids)}")
        
        # Show all markets with their update counts
        print("\nUpdates per market:")
        sorted_markets = sorted(updates_count.items(), key=lambda x: x[1], reverse=True)
        for mid, count in sorted_markets:
            sym = get_symbol(mid)
            rate = count / elapsed
            print(f"  {sym:8s} (ID:{mid:3d}): {count:6d} updates ({rate:6.1f}/sec)")

def main():
    import argparse
    parser = argparse.ArgumentParser(description='Stream all active Hyperliquid markets')
    parser.add_argument('--host', default='localhost', help='Server host')
    parser.add_argument('--port', type=int, default=50052, help='Server port')
    parser.add_argument('--max-markets', type=int, help='Limit number of markets to stream')
    parser.add_argument('--list-markets', action='store_true', help='Just list all available markets')
    
    args = parser.parse_args()
    
    if args.list_markets:
        print("All available markets:")
        print(f"{'ID':>4} | {'Symbol':<10} | {'Example Usage'}")
        print("-" * 50)
        for symbol, market_id in sorted(MARKET_IDS.items(), key=lambda x: x[1]):
            print(f"{market_id:4d} | {symbol:<10} | --markets {market_id}")
        print(f"\nTotal: {len(MARKET_IDS)} active markets")
    else:
        stream_all_markets(args.host, args.port, args.max_markets)

if __name__ == "__main__":
    main()