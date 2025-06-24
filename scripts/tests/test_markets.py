#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc

def test_markets():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    print("Getting available markets...")
    
    try:
        request = orderbook_pb2.GetMarketsRequest()
        response = stub.GetMarkets(request)
        
        print(f"\nFound {len(response.markets)} markets:")
        for market in response.markets:
            print(f"  - Market ID: {market.market_id}, Symbol: {market.symbol}, Active: {market.active}")
            
            # Get orderbook for each active market
            if market.active:
                ob_request = orderbook_pb2.GetOrderbookRequest(
                    market_id=market.market_id,
                    depth=5
                )
                snapshot = stub.GetOrderbook(ob_request)
                
                bid_count = len(snapshot.bids)
                ask_count = len(snapshot.asks)
                
                if bid_count > 0 or ask_count > 0:
                    print(f"    Orderbook: {bid_count} bids, {ask_count} asks")
                    if bid_count > 0 and ask_count > 0:
                        best_bid = snapshot.bids[0].price
                        best_ask = snapshot.asks[0].price
                        print(f"    Best Bid: ${best_bid:,.2f}, Best Ask: ${best_ask:,.2f}")
        
        return True
        
    except grpc.RpcError as e:
        print(f"RPC Error: {e.code()} - {e.details()}")
        return False

if __name__ == "__main__":
    test_markets()