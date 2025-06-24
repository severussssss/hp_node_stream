#!/usr/bin/env python3
import grpc
import orderbook_pb2
import orderbook_pb2_grpc
import time

def test_polling():
    channel = grpc.insecure_channel('localhost:50051')
    stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
    
    print("Testing orderbook service by polling snapshots...")
    print("This simulates real-time updates by polling every second")
    print("-" * 60)
    
    prev_sequences = {}
    
    for i in range(10):
        try:
            # Get snapshots for both markets
            for market_id in [0, 159]:
                request = orderbook_pb2.GetOrderbookRequest(
                    market_id=market_id,
                    depth=5
                )
                
                snapshot = stub.GetOrderbook(request)
                
                # Check if sequence changed (indicating an update)
                prev_seq = prev_sequences.get(market_id, -1)
                is_update = snapshot.sequence != prev_seq
                prev_sequences[market_id] = snapshot.sequence
                
                if i == 0 or is_update:
                    print(f"\nPoll #{i+1} - {snapshot.symbol} (Market {market_id}):")
                    print(f"  Sequence: {snapshot.sequence} {'(NEW)' if is_update and i > 0 else ''}")
                    print(f"  Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
                    
                    if snapshot.bids and snapshot.asks:
                        best_bid = snapshot.bids[0].price
                        best_ask = snapshot.asks[0].price
                        spread = best_ask - best_bid
                        print(f"  Best Bid: ${best_bid:,.2f}")
                        print(f"  Best Ask: ${best_ask:,.2f}")
                        print(f"  Spread: ${spread:.2f}")
            
            time.sleep(1)
            
        except grpc.RpcError as e:
            print(f"RPC Error: {e.code()} - {e.details()}")
            return False
    
    print("\n" + "-" * 60)
    print("✓ Polling test completed successfully!")
    print("The orderbook service is responding to snapshot requests.")
    return True

if __name__ == "__main__":
    test_polling()