#!/usr/bin/env python3
"""
Hyperliquid Node Orderbook Client

Real-time streaming client for the orderbook service with Hyperliquid-style UX.
Supports multiple authentication methods for secure remote access.
"""
import asyncio
import grpc
import sys
import time
import os
import ssl
import json
from typing import List, Dict, Optional
from datetime import datetime
import signal

# Import market config for symbol resolution
from market_config import get_market_id

# Import the generated protobuf modules
sys.path.insert(0, 'proto')
try:
    import orderbook_pb2
    import orderbook_pb2_grpc
except ImportError:
    print("Error: Could not import proto modules.")
    print("Make sure the proto files are generated in the proto/ directory")
    sys.exit(1)


class AuthCredentials:
    """Container for authentication credentials"""
    def __init__(self):
        self.api_key: Optional[str] = None
        self.jwt_token: Optional[str] = None
        self.tls_cert: Optional[str] = None
        self.tls_key: Optional[str] = None
        self.tls_ca: Optional[str] = None


class RealtimeOrderbookClient:
    """Client for real-time orderbook streaming service with in-place updates"""
    
    def __init__(self, server_address: str = "localhost:50052", auth: Optional[AuthCredentials] = None):
        self.server_address = server_address
        self.auth = auth
        self.channel = None
        self.stub = None
        self._running = True

    async def connect(self):
        """Establish connection to the gRPC server with authentication."""
        if self.auth and (self.auth.tls_cert or self.auth.tls_ca):
            # TLS/mTLS connection
            credentials = self._create_tls_credentials()
            
            # Add call credentials if API key or JWT provided
            if self.auth.api_key or self.auth.jwt_token:
                call_creds = self._create_call_credentials()
                credentials = grpc.composite_channel_credentials(credentials, call_creds)
            
            self.channel = grpc.aio.secure_channel(self.server_address, credentials)
        else:
            # Insecure connection (with optional API key/JWT)
            if self.auth and (self.auth.api_key or self.auth.jwt_token):
                # Create insecure channel with call credentials
                call_creds = self._create_call_credentials()
                # For insecure channel with auth, we need to use metadata
                self.channel = grpc.aio.insecure_channel(self.server_address)
            else:
                self.channel = grpc.aio.insecure_channel(self.server_address)
        
        self.stub = orderbook_pb2_grpc.OrderbookServiceStub(self.channel)
    
    def _create_tls_credentials(self) -> grpc.ChannelCredentials:
        """Create TLS credentials from certificates."""
        # Read CA certificate
        ca_cert = None
        if self.auth.tls_ca:
            with open(self.auth.tls_ca, 'rb') as f:
                ca_cert = f.read()
        
        # Read client certificate and key for mTLS
        client_cert = None
        client_key = None
        if self.auth.tls_cert and self.auth.tls_key:
            with open(self.auth.tls_cert, 'rb') as f:
                client_cert = f.read()
            with open(self.auth.tls_key, 'rb') as f:
                client_key = f.read()
        
        return grpc.ssl_channel_credentials(
            root_certificates=ca_cert,
            private_key=client_key,
            certificate_chain=client_cert
        )
    
    def _create_call_credentials(self) -> grpc.CallCredentials:
        """Create call credentials for API key or JWT."""
        def auth_plugin(context, callback):
            metadata = []
            if self.auth.api_key:
                metadata.append(('x-api-key', self.auth.api_key))
            if self.auth.jwt_token:
                metadata.append(('authorization', f'Bearer {self.auth.jwt_token}'))
            callback(metadata, None)
        
        return grpc.metadata_call_credentials(auth_plugin)
    
    def _create_metadata(self) -> List[tuple]:
        """Create metadata for authentication."""
        metadata = []
        if self.auth:
            if self.auth.api_key:
                metadata.append(('x-api-key', self.auth.api_key))
            if self.auth.jwt_token:
                metadata.append(('authorization', f'Bearer {self.auth.jwt_token}'))
        return metadata

    async def close(self):
        """Close the gRPC channel."""
        if self.channel:
            await self.channel.close()

    def _format_number(self, num: float, decimals: int = 2) -> str:
        """Format number with thousands separator."""
        if decimals == 0:
            return f"{int(num):,}"
        return f"{num:,.{decimals}f}"

    def _get_color_code(self, is_bid: bool) -> str:
        """Get ANSI color code for bids/asks."""
        return "\033[92m" if is_bid else "\033[91m"  # Green for bids, Red for asks

    def _reset_color(self) -> str:
        """Reset ANSI color."""
        return "\033[0m"

    def _display_orderbooks(self, snapshots: Dict[int, any], update_count: int, elapsed: float):
        """Display all orderbooks in a grid layout."""
        # Clear screen and move cursor to top
        print("\033[2J\033[H", end="")
        
        # Calculate update rate
        update_rate = update_count / elapsed if elapsed > 0 else 0
        
        # Header
        print(f"REAL-TIME ORDERBOOK | Updates: {update_count:,} | Rate: {update_rate:.0f}/sec | {datetime.now().strftime('%H:%M:%S')}")
        print("=" * 160)
        
        # Display each market
        for market_id in sorted(snapshots.keys()):
            snapshot = snapshots[market_id]
            self._display_single_orderbook(snapshot)
            print("-" * 160)

    def _display_single_orderbook(self, snapshot):
        """Display a single orderbook in Hyperliquid style."""
        # Market header
        if snapshot.bids and snapshot.asks:
            best_bid = snapshot.bids[0].price
            best_ask = snapshot.asks[0].price
            spread = best_ask - best_bid
            spread_bps = (spread / best_bid) * 10000 if best_bid > 0 else 0
            mid = (best_bid + best_ask) / 2
            
            # Market title bar
            print(f"\n{snapshot.symbol} | "
                  f"Mid: ${self._format_number(mid, 2)} | "
                  f"Spread: ${spread:.2f} ({spread_bps:.1f} bps) | "
                  f"Seq: {snapshot.sequence}")
        else:
            print(f"\n{snapshot.symbol} | Seq: {snapshot.sequence}")
        
        # Column headers
        print(f"\n{'':>15}{'BID SIZE':>15}{'BID PRICE':>15}{'':^10}{'ASK PRICE':>15}{'ASK SIZE':>15}")
        print(f"{'-'*15}{'-'*15:>15}{'-'*15:>15}{'':^10}{'-'*15}{'-'*15:>15}")
        
        # Display orderbook levels (20 levels)
        max_bid_size = max([b.quantity for b in snapshot.bids[:20]], default=1)
        max_ask_size = max([a.quantity for a in snapshot.asks[:20]], default=1)
        
        for i in range(20):
            # Bid side
            if i < len(snapshot.bids):
                bid = snapshot.bids[i]
                bid_bar_width = int((bid.quantity / max_bid_size) * 15)
                bid_bar = "█" * bid_bar_width
                bid_price_str = f"{self._get_color_code(True)}{self._format_number(bid.price, 2)}{self._reset_color()}"
                bid_size_str = self._format_number(bid.quantity, 4)
                bid_line = f"{bid_bar:>15}{bid_size_str:>15}{bid_price_str:>15}"
            else:
                bid_line = " " * 45
            
            # Ask side
            if i < len(snapshot.asks):
                ask = snapshot.asks[i]
                ask_bar_width = int((ask.quantity / max_ask_size) * 15)
                ask_bar = "█" * ask_bar_width
                ask_price_str = f"{self._get_color_code(False)}{self._format_number(ask.price, 2)}{self._reset_color()}"
                ask_size_str = self._format_number(ask.quantity, 4)
                ask_line = f"{ask_price_str:>15}{ask_size_str:>15}{ask_bar:<15}"
            else:
                ask_line = " " * 45
            
            # Print the level
            print(f"{bid_line}{'':^10}{ask_line}")
        
        # Summary stats
        if snapshot.bids and snapshot.asks:
            total_bid_size = sum(b.quantity for b in snapshot.bids[:20])
            total_ask_size = sum(a.quantity for a in snapshot.asks[:20])
            
            print(f"\n{'Total (20 levels):':>30} "
                  f"{self._get_color_code(True)}{self._format_number(total_bid_size, 4):>15}{self._reset_color()} "
                  f"{'':^25}"
                  f"{self._get_color_code(False)}{self._format_number(total_ask_size, 4):>15}{self._reset_color()}")

    async def subscribe_orderbook(self, market_ids: List[int]):
        """Subscribe to real-time orderbook updates with in-place display."""
        if not self.stub:
            await self.connect()

        request = orderbook_pb2.SubscribeRequest(market_ids=market_ids)
        
        print(f"Connecting to {self.server_address}...")
        if self.auth:
            auth_methods = []
            if self.auth.api_key:
                auth_methods.append("API Key")
            if self.auth.jwt_token:
                auth_methods.append("JWT")
            if self.auth.tls_cert:
                auth_methods.append("mTLS")
            elif self.auth.tls_ca:
                auth_methods.append("TLS")
            print(f"Authentication: {', '.join(auth_methods)}")
        print(f"Subscribing to markets: {market_ids}")
        print("\nPress Ctrl+C to stop\n")
        
        update_count = 0
        start_time = time.time()
        last_display_time = time.time()
        
        # Store the latest snapshot for each market
        market_snapshots = {}
        
        # Hide cursor for cleaner display
        print("\033[?25l", end="")
        
        try:
            # Add metadata for authentication if needed
            metadata = self._create_metadata()
            async for snapshot in self.stub.SubscribeOrderbook(request, metadata=metadata):
                if not self._running:
                    break
                    
                update_count += 1
                market_snapshots[snapshot.market_id] = snapshot
                
                # Update display every 100ms for smooth updates
                current_time = time.time()
                if current_time - last_display_time >= 0.1:
                    self._display_orderbooks(market_snapshots, update_count, current_time - start_time)
                    last_display_time = current_time
                    
        except grpc.RpcError as e:
            print(f"\nRPC Error: {e.code()}: {e.details()}")
        except KeyboardInterrupt:
            pass
        finally:
            # Show cursor again
            print("\033[?25h", end="")
            print("\n\nStopping stream...")

    def stop(self):
        """Stop the client."""
        self._running = False


async def stream_mode(server_address: str, market_ids: List[int], auth: Optional[AuthCredentials] = None):
    """Stream orderbook updates for specified markets."""
    client = RealtimeOrderbookClient(server_address, auth)
    
    # Setup signal handler for clean shutdown
    def signal_handler(sig, frame):
        client.stop()
    
    signal.signal(signal.SIGINT, signal_handler)
    
    try:
        await client.subscribe_orderbook(market_ids)
    finally:
        await client.close()


def main():
    """Main entry point."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description="Hyperliquid Node Orderbook Client - Stream real-time L2 orderbook data",
        epilog="""Authentication options:
  API Key:   Set ORDERBOOK_API_KEY environment variable or use --api-key
  JWT Token: Set ORDERBOOK_JWT_TOKEN environment variable or use --jwt-token
  TLS:       Use --tls-ca for server verification
  mTLS:      Use --tls-cert, --tls-key, and --tls-ca for mutual TLS
  
Examples:
  # Connect to local server without auth
  %(prog)s HYPERLIQUID-BTC/USD-PERP HYPERLIQUID-SOL/USD-PERP
  
  # Connect to remote server with API key
  %(prog)s --host api.example.com --port 443 --api-key YOUR_KEY HYPERLIQUID-BTC/USD-PERP HYPERLIQUID-ETH/USD-PERP
  
  # Connect with TLS
  %(prog)s --host api.example.com --port 443 --tls-ca ca.crt HYPERLIQUID-BTC/USD-PERP
  
  # Connect with mTLS
  %(prog)s --host api.example.com --port 443 --tls-cert client.crt --tls-key client.key --tls-ca ca.crt HYPERLIQUID-BTC/USD-PERP
  
  # Using environment variables
  export ORDERBOOK_API_KEY=YOUR_KEY
  %(prog)s --host api.example.com HYPERLIQUID-BTC/USD-PERP HYPERLIQUID-ETH/USD-PERP
        """,
        formatter_class=argparse.RawDescriptionHelpFormatter
    )
    
    parser.add_argument("symbols", nargs="+", type=str, 
                        help="Market symbols to stream. Supports full format (HYPERLIQUID-BTC/USD-PERP) or simple (BTC)")
    parser.add_argument("--host", default="localhost", help="Server host (default: localhost)")
    parser.add_argument("--port", default=50052, type=int, help="Server port (default: 50052)")
    
    # Authentication options
    auth_group = parser.add_argument_group('authentication')
    auth_group.add_argument("--api-key", help="API key for authentication")
    auth_group.add_argument("--jwt-token", help="JWT token for authentication")
    auth_group.add_argument("--tls-ca", help="CA certificate file for TLS verification")
    auth_group.add_argument("--tls-cert", help="Client certificate file for mTLS")
    auth_group.add_argument("--tls-key", help="Client key file for mTLS")
    
    # Configuration file option
    parser.add_argument("--config", help="JSON config file with connection settings")
    
    args = parser.parse_args()
    
    # Load config from file if provided
    if args.config:
        with open(args.config, 'r') as f:
            config = json.load(f)
            # Override args with config values
            args.host = config.get('host', args.host)
            args.port = config.get('port', args.port)
            args.api_key = config.get('api_key', args.api_key)
            args.jwt_token = config.get('jwt_token', args.jwt_token)
            args.tls_ca = config.get('tls_ca', args.tls_ca)
            args.tls_cert = config.get('tls_cert', args.tls_cert)
            args.tls_key = config.get('tls_key', args.tls_key)
    
    # Check environment variables if not provided via args
    if not args.api_key:
        args.api_key = os.environ.get('ORDERBOOK_API_KEY')
    if not args.jwt_token:
        args.jwt_token = os.environ.get('ORDERBOOK_JWT_TOKEN')
    
    # Validate mTLS arguments
    if args.tls_cert and not args.tls_key:
        parser.error("--tls-cert requires --tls-key")
    if args.tls_key and not args.tls_cert:
        parser.error("--tls-key requires --tls-cert")
    
    # Build server address
    server_address = f"{args.host}:{args.port}"
    
    # Convert symbols to market IDs
    market_ids = []
    for symbol in args.symbols:
        market_id = get_market_id(symbol)
        if market_id is not None:
            market_ids.append(market_id)
        else:
            print(f"Warning: Unknown symbol '{symbol}'")
    
    if not market_ids:
        print("Error: No valid symbols provided")
        sys.exit(1)
    
    # Create auth credentials
    auth = None
    if any([args.api_key, args.jwt_token, args.tls_ca, args.tls_cert]):
        auth = AuthCredentials()
        auth.api_key = args.api_key
        auth.jwt_token = args.jwt_token
        auth.tls_ca = args.tls_ca
        auth.tls_cert = args.tls_cert
        auth.tls_key = args.tls_key
    
    # Run streaming mode
    asyncio.run(stream_mode(server_address, market_ids, auth))


if __name__ == "__main__":
    main()