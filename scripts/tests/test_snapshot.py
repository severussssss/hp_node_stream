#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc

def test_snapshot():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    # Get orderbook snapshot for BTC-PERP
    request = orderbook_pb2.GetOrderbookRequest(
        market_id=0,
        depth=10
    )
    
    print("Getting orderbook snapshot for BTC-PERP (market_id=0)...")
    
    try:
        snapshot = stub.GetOrderbook(request)
        
        print(f"\nMarket: {snapshot.symbol} (ID: {snapshot.market_id})")
        print(f"Sequence: {snapshot.sequence}")
        print(f"Timestamp: {snapshot.timestamp_us}")
        
        if snapshot.bids:
            print(f"\nBids (top {len(snapshot.bids)}):")
            for i, bid in enumerate(snapshot.bids):
                print(f"  {i+1}. Price: ${bid.price:,.2f}, Qty: {bid.quantity:.4f}, Orders: {bid.order_count}")
        else:
            print("\nNo bids in orderbook")
        
        if snapshot.asks:
            print(f"\nAsks (top {len(snapshot.asks)}):")
            for i, ask in enumerate(snapshot.asks):
                print(f"  {i+1}. Price: ${ask.price:,.2f}, Qty: {ask.quantity:.4f}, Orders: {ask.order_count}")
        else:
            print("\nNo asks in orderbook")
            
        if snapshot.bids and snapshot.asks:
            spread = snapshot.asks[0].price - snapshot.bids[0].price
            mid = (snapshot.asks[0].price + snapshot.bids[0].price) / 2
            print(f"\nSpread: ${spread:.2f}")
            print(f"Mid Price: ${mid:,.2f}")
        
        return True
        
    except grpc.RpcError as e:
        print(f"RPC Error: {e.code()} - {e.details()}")
        return False

if __name__ == "__main__":
    test_snapshot()