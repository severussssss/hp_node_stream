#!/usr/bin/env python3
import grpc
import subscribe_pb2
import subscribe_pb2_grpc
import time
import sys
import asyncio

def format_price(price, decimals=2):
    """Format price with appropriate decimals"""
    if price > 1000:
        return f"${price:,.{decimals}f}"
    elif price > 10:
        return f"${price:.{decimals}f}"
    else:
        return f"${price:.{decimals+2}f}"

def display_mark_price(market_id, symbol, port=50053):
    """Display mark price calculations for a market"""
    channel = grpc.insecure_channel(f'localhost:{port}')
    stub = subscribe_pb2_grpc.OrderbookServiceStub(channel)
    
    print(f"\nMark Price Monitor - {symbol}")
    print("=" * 80)
    print("Press Ctrl+C to exit\n")
    
    try:
        while True:
            # Get orderbook snapshot with mark price
            request = subscribe_pb2.GetOrderbookRequest(market_id=market_id, depth=10)
            response = stub.GetOrderbook(request)
            
            if response.bids and response.asks:
                best_bid = response.bids[0].price
                best_ask = response.asks[0].price
                spread = best_ask - best_bid
                spread_bps = (spread / ((best_bid + best_ask) / 2)) * 10000
                
                # Clear screen for clean display
                print("\033[H\033[J", end='')
                
                print(f"Mark Price Monitor - {symbol}")
                print("=" * 80)
                print(f"Time: {time.strftime('%Y-%m-%d %H:%M:%S')}")
                print()
                
                # Display orderbook top
                print("Orderbook:")
                print(f"  Best Bid: {format_price(best_bid)} x {response.bids[0].quantity:.4f}")
                print(f"  Best Ask: {format_price(best_ask)} x {response.asks[0].quantity:.4f}")
                print(f"  Spread:   {format_price(spread, 4)} ({spread_bps:.1f} bps)")
                print()
                
                # Display mark price if available
                if response.HasField('mark_price'):
                    mp = response.mark_price
                    
                    print("Mark Price Calculation:")
                    print(f"  Mid Price:        {format_price(mp.mid_price)}")
                    print(f"  Impact Bid:       {format_price(mp.impact_bid_price)} (buy impact)")
                    print(f"  Impact Ask:       {format_price(mp.impact_ask_price)} (sell impact)")
                    print(f"  Impact Mid:       {format_price(mp.impact_mid_price)}")
                    print(f"  EMA Price:        {format_price(mp.ema_price)}")
                    print(f"  Confidence:       {mp.confidence:.1%}")
                    print()
                    print(f"  ► MARK PRICE:     {format_price(mp.mark_price)}")
                    
                    # Calculate deviations
                    mark_vs_mid = ((mp.mark_price - mp.mid_price) / mp.mid_price) * 10000
                    impact_skew = ((mp.impact_ask_price - mp.impact_bid_price) / mp.impact_mid_price) * 10000
                    
                    print()
                    print("Analysis:")
                    print(f"  Mark vs Mid:      {mark_vs_mid:+.1f} bps")
                    print(f"  Impact Skew:      {impact_skew:.1f} bps")
                    
                    # Show top 5 levels for context
                    print("\nOrderbook Depth:")
                    print("  BIDS                          ASKS")
                    print("  " + "-" * 30 + "  " + "-" * 30)
                    for i in range(min(5, len(response.bids), len(response.asks))):
                        bid = response.bids[i]
                        ask = response.asks[i]
                        print(f"  {bid.quantity:>8.4f} @ {format_price(bid.price):<12} {ask.quantity:>8.4f} @ {format_price(ask.price)}")
                else:
                    print("Mark price not available yet...")
            else:
                print("Orderbook empty...")
            
            time.sleep(1)
            
    except KeyboardInterrupt:
        print("\nExiting...")
    except grpc.RpcError as e:
        print(f"Error: {e.code()} - {e.details()}")

async def stream_mark_prices(markets, port=50053):
    """Stream mark prices for multiple markets"""
    channel = grpc.insecure_channel(f'localhost:{port}')
    stub = subscribe_pb2_grpc.OrderbookServiceStub(channel)
    
    # Subscribe to markets
    request = subscribe_pb2.SubscribeRequest(
        market_ids=[m[0] for m in markets],
        depth=5,
        update_interval_ms=1000
    )
    
    print("\nMark Price Streaming Monitor")
    print("=" * 80)
    print(f"{'Symbol':<8} {'Mid Price':>12} {'Mark Price':>12} {'Impact Bid':>12} {'Impact Ask':>12} {'Confidence':>10}")
    print("-" * 80)
    
    try:
        stream = stub.SubscribeOrderbook(request)
        for snapshot in stream:
            # Find symbol name
            symbol = next((m[1] for m in markets if m[0] == snapshot.market_id), "???")
            
            if snapshot.bids and snapshot.asks and snapshot.HasField('mark_price'):
                mp = snapshot.mark_price
                mid = (snapshot.bids[0].price + snapshot.asks[0].price) / 2
                
                # Update line for this market
                print(f"\r{symbol:<8} {format_price(mid):>12} {format_price(mp.mark_price):>12} "
                      f"{format_price(mp.impact_bid_price):>12} {format_price(mp.impact_ask_price):>12} "
                      f"{mp.confidence:>9.1%}", end='', flush=True)
                
                # Move to next line for next update
                print()
    except KeyboardInterrupt:
        print("\nExiting...")
    except grpc.RpcError as e:
        print(f"Error: {e.code()} - {e.details()}")

def main():
    if len(sys.argv) > 1 and sys.argv[1] == 'stream':
        # Stream multiple markets
        markets = [
            (0, "BTC"),
            (1, "ETH"),
            (5, "SOL"),
            (11, "ARB"),
            (159, "HYPE")
        ]
        asyncio.run(stream_mark_prices(markets))
    else:
        # Monitor single market
        market_id = int(sys.argv[1]) if len(sys.argv) > 1 else 0
        symbol = sys.argv[2] if len(sys.argv) > 2 else "BTC"
        port = int(sys.argv[3]) if len(sys.argv) > 3 else 50053
        
        display_mark_price(market_id, symbol, port)

if __name__ == '__main__':
    main()