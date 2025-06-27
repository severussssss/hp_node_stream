#!/usr/bin/env python3
"""
Test script for external EC2 connection to orderbook service
"""
import grpc
import time
import sys
import subscribe_pb2
import subscribe_pb2_grpc
from datetime import datetime

def test_connection(server_address, use_tls=False, api_key=None):
    """Test connection from external EC2"""
    
    print(f"Testing connection to {server_address}")
    print(f"TLS: {use_tls}, API Key: {'Yes' if api_key else 'No'}\n")
    
    # Configure channel options for long-lived connections
    options = [
        ('grpc.keepalive_time_ms', 10000),
        ('grpc.keepalive_timeout_ms', 5000),
        ('grpc.keepalive_permit_without_calls', True),
        ('grpc.http2.max_pings_without_data', 0),
        ('grpc.max_receive_message_length', 10 * 1024 * 1024),  # 10MB
    ]
    
    try:
        # Create channel
        if use_tls:
            # Load CA certificate
            with open('ca-cert.pem', 'rb') as f:
                ca_cert = f.read()
            creds = grpc.ssl_channel_credentials(root_certificates=ca_cert)
            channel = grpc.secure_channel(server_address, creds, options=options)
        else:
            channel = grpc.insecure_channel(server_address, options=options)
        
        # Add API key interceptor if provided
        if api_key:
            from functools import partial
            
            class ApiKeyInterceptor(grpc.UnaryUnaryClientInterceptor,
                                   grpc.UnaryStreamClientInterceptor):
                def __init__(self, api_key):
                    self.api_key = api_key
                
                def _add_metadata(self, client_call_details):
                    metadata = list(client_call_details.metadata or [])
                    metadata.append(('x-api-key', self.api_key))
                    return client_call_details._replace(metadata=metadata)
                
                def intercept_unary_unary(self, continuation, client_call_details, request):
                    new_details = self._add_metadata(client_call_details)
                    return continuation(new_details, request)
                
                def intercept_unary_stream(self, continuation, client_call_details, request):
                    new_details = self._add_metadata(client_call_details)
                    return continuation(new_details, request)
            
            channel = grpc.intercept_channel(channel, ApiKeyInterceptor(api_key))
        
        stub = subscribe_pb2_grpc.OrderbookServiceStub(channel)
        
        # Test 1: Get Markets
        print("1. Testing GetMarkets...")
        try:
            markets = stub.GetMarkets(subscribe_pb2.Empty(), timeout=5)
            print(f"✓ Found {len(markets.markets)} markets")
            for market in markets.markets[:5]:
                print(f"   Market {market.id}: {market.symbol}")
        except grpc.RpcError as e:
            print(f"✗ GetMarkets failed: {e.code()} - {e.details()}")
            return False
        
        # Test 2: Get Single Orderbook
        print("\n2. Testing GetOrderbook...")
        try:
            request = subscribe_pb2.GetOrderbookRequest(market_id=0, depth=10)
            snapshot = stub.GetOrderbook(request, timeout=5)
            print(f"✓ Got orderbook for {snapshot.symbol}")
            print(f"   Bids: {len(snapshot.bids)}, Asks: {len(snapshot.asks)}")
            if snapshot.bids and snapshot.asks:
                print(f"   Best bid: ${snapshot.bids[0].price:.2f}")
                print(f"   Best ask: ${snapshot.asks[0].price:.2f}")
        except grpc.RpcError as e:
            print(f"✗ GetOrderbook failed: {e.code()} - {e.details()}")
            return False
        
        # Test 3: Stream Orderbook Updates
        print("\n3. Testing SubscribeOrderbook (5 second stream)...")
        try:
            request = subscribe_pb2.SubscribeRequest(
                market_ids=[0, 1],  # BTC, ETH
                depth=5,
                update_interval_ms=1000
            )
            
            stream = stub.SubscribeOrderbook(request, timeout=10)
            
            start_time = time.time()
            update_count = 0
            
            for snapshot in stream:
                update_count += 1
                timestamp = datetime.now().strftime('%H:%M:%S.%f')[:-3]
                
                print(f"[{timestamp}] Update #{update_count} for {snapshot.symbol}")
                print(f"   Sequence: {snapshot.sequence}")
                print(f"   Best bid: ${snapshot.bids[0].price:.2f} x {snapshot.bids[0].quantity:.4f}")
                print(f"   Best ask: ${snapshot.asks[0].price:.2f} x {snapshot.asks[0].quantity:.4f}")
                
                if time.time() - start_time > 5:
                    break
            
            print(f"\n✓ Received {update_count} updates in 5 seconds")
            
        except grpc.RpcError as e:
            print(f"✗ SubscribeOrderbook failed: {e.code()} - {e.details()}")
            return False
        
        # Test 4: Mark Price Stream (if available)
        print("\n4. Testing SubscribeMarkPrices...")
        try:
            request = subscribe_pb2.MarkPriceSubscribeRequest(
                market_ids=[0],
                update_interval_ms=1000
            )
            
            stream = stub.SubscribeMarkPrices(request, timeout=5)
            
            update = next(stream)
            print(f"✓ Got mark price for {update.symbol}: ${update.hl_mark_price.mark_price:.2f}")
            
        except grpc.RpcError as e:
            if e.code() == grpc.StatusCode.UNIMPLEMENTED:
                print("✓ Mark price endpoint not implemented (expected for old binary)")
            else:
                print(f"✗ SubscribeMarkPrices failed: {e.code()} - {e.details()}")
        
        print("\n✅ All tests passed! External connection successful.")
        return True
        
    except Exception as e:
        print(f"\n❌ Connection failed: {e}")
        return False
    finally:
        channel.close()

def main():
    """Main function"""
    if len(sys.argv) < 2:
        print("Usage: python test_external_connection.py <server:port> [--tls] [--api-key KEY]")
        print("\nExamples:")
        print("  python test_external_connection.py 54.123.45.67:50053")
        print("  python test_external_connection.py orderbook.example.com:50053 --tls")
        print("  python test_external_connection.py 10.0.1.23:50053 --api-key abc123")
        sys.exit(1)
    
    server = sys.argv[1]
    use_tls = '--tls' in sys.argv
    
    api_key = None
    if '--api-key' in sys.argv:
        key_index = sys.argv.index('--api-key')
        if key_index + 1 < len(sys.argv):
            api_key = sys.argv[key_index + 1]
    
    # Test connection
    success = test_connection(server, use_tls, api_key)
    sys.exit(0 if success else 1)

if __name__ == "__main__":
    main()