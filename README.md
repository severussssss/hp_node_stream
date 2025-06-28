# Orderbook Service

A high-performance gRPC orderbook streaming service with real-time updates from Hyperliquid.

## Features

- Real-time L2 orderbook streaming for 199 Hyperliquid markets
- Lock-free data structures for high performance
- Separate mark price calculation service (1Hz updates)
- Stop order tracking and filtering
- Robust JSON parsing with error recovery
- Multiple authentication options (API Key, JWT, TLS/mTLS)
- Python client with Hyperliquid-style UX

## Client Applications

- `node_client.py` - Main orderbook streaming client with authentication support
- `mark_price_client.py` - Specialized client for monitoring mark prices
- `test_external_connection.py` - Utility for testing connectivity and authentication

Real-time orderbook streaming service built in Rust with Python client libraries.

## Overview

This service provides real-time Level 2 (L2) orderbook data from Hyperliquid markets via gRPC. It processes order updates from the Hyperliquid node and maintains in-memory orderbooks with microsecond-level performance.

## Features

- **Real-time L2 orderbook streaming** with full market depth
- **High performance**: 700+ updates/second throughput
- **Multiple market support**: BTC, ETH, SOL, and more
- **Trigger order filtering**: Excludes stop/trigger orders from the orderbook
- **gRPC API** for efficient streaming
- **In-place orderbook updates** (Hyperliquid-style UX)

## Architecture

- **Rust service** (`orderbook-service-realtime`): Processes orders and maintains orderbooks
- **Python clients**: Multiple client implementations for different use cases
- **gRPC protocol**: Efficient binary protocol for streaming

## Building the Service

### Prerequisites

- Rust 1.70+ 
- Cargo
- Access to Hyperliquid node data

### Build

```bash
cargo build --release --bin orderbook-service-realtime
```

### Run

```bash
# Run with default port (50052)
./target/release/orderbook-service-realtime

# Run with custom port
./target/release/orderbook-service-realtime --grpc-port 50053

# Run with logging
RUST_LOG=info ./target/release/orderbook-service-realtime --grpc-port 50053
```

## Python Clients

### Installation

```bash
pip install -r requirements.txt
```

### Available Clients

#### 1. Node Client (`node_client.py`)

The main client with Hyperliquid-style orderbook display that updates in place. Supports authentication for remote access.

```bash
# Local connection (BTC and SOL)
python3 node_client.py 0 6

# Remote with authentication
python3 node_client.py --host api.example.com --port 443 --api-key YOUR_KEY 0 6

# Stream all major markets
python3 node_client.py 0 1 2 3 4 5 6 7 8 9
```

#### 2. Mark Price Client (`mark_price_client.py`)

Specialized client for monitoring mark price calculations.

```bash
# Monitor BTC mark price
python3 mark_price_client.py 0 BTC

# Monitor multiple markets
python3 mark_price_client.py 0 BTC 6 SOL
```

#### 3. Connection Test Utility (`test_external_connection.py`)

Test connectivity and authentication from external servers.

```bash
# Test basic connectivity
python3 test_external_connection.py --host api.example.com --port 443

# Test with authentication
python3 test_external_connection.py --host api.example.com --port 443 --api-key YOUR_KEY
```

## Market IDs

The service supports all 199 Hyperliquid perpetual markets. To see the full list:

```bash
python3 list_markets.py --port 50053
```

### Popular Market IDs and Symbols

**Major Markets:**
- 0: HYPERLIQUID-BTC/USD-PERP
- 1: HYPERLIQUID-ETH/USD-PERP  
- 5: HYPERLIQUID-SOL/USD-PERP (note: not 6!)
- 11: HYPERLIQUID-ARB/USD-PERP (note: not 2!)
- 13: HYPERLIQUID-OP/USD-PERP
- 6: HYPERLIQUID-AVAX/USD-PERP
- 3: HYPERLIQUID-MATIC/USD-PERP
- 2: HYPERLIQUID-ATOM/USD-PERP
- 9: HYPERLIQUID-DOGE/USD-PERP
- 10: HYPERLIQUID-BNB/USD-PERP

**Meme Coins:**
- 98: HYPERLIQUID-WIF/USD-PERP
- 96: HYPERLIQUID-PEPE/USD-PERP
- 48: HYPERLIQUID-BONK/USD-PERP
- 79: HYPERLIQUID-POPCAT/USD-PERP
- 93: HYPERLIQUID-MEW/USD-PERP
- 85: HYPERLIQUID-BRETT/USD-PERP
- 100: HYPERLIQUID-MOG/USD-PERP
- 125: HYPERLIQUID-NEIRO/USD-PERP
- 130: HYPERLIQUID-GOAT/USD-PERP

**New Listings:**
- 159: HYPERLIQUID-HYPE/USD-PERP
- 162: HYPERLIQUID-PENGU/USD-PERP
- 163: HYPERLIQUID-MOVE/USD-PERP
- 170: HYPERLIQUID-SONIC/USD-PERP

**Note:** The market IDs are assigned chronologically as markets are listed, not alphabetically.

## Example Output

### L2 Orderbook Display

```
HYPERLIQUID-BTC/USD-PERP | Mid: $105,489.50 | Spread: $1.00 (0.1 bps) | Seq: 47445

                 BIDS                  |                  ASKS                 
-------------------------------------- | --------------------------------------
      1.1485 @  105,489.00            |  105,490.00 @       0.0001            
      4.6889 @  105,488.00            |  105,491.00 @       0.0001            
      0.0001 @  105,487.00            |  105,492.00 @       0.0001            
      0.1845 @  105,486.00            |  105,493.00 @       0.0001            
      0.0001 @  105,485.00            |  105,494.00 @       0.0001            
```

## Protocol Buffer Definition

The service uses Protocol Buffers for message serialization. See `subscribe.proto` for the complete API definition.

### Key Messages

- `SubscribeRequest`: Subscribe to orderbook updates for specific markets
- `OrderbookSnapshot`: Full orderbook state with bids/asks
- `GetOrderbookRequest`: Request a single orderbook snapshot
- `GetMarketsRequest`: List available markets

## Performance

- **Update Rate**: 700+ updates/second per market
- **Latency**: Sub-millisecond orderbook updates
- **Memory**: ~100MB for 10 markets with full depth

## Configuration

### Service Configuration

- **Default gRPC Port**: 50052
- **Data Source**: Reads from Hyperliquid node data at `/home/hluser/hl/data/node_order_statuses/hourly/`
- **Update Channel Size**: 100,000 messages

### Client Configuration

All clients support:
- `--host`: Server hostname (default: localhost)
- `--port`: Server port (default: 50052)

## Important Notes

1. **Data Freshness**: The service reads from hourly data files. Restart the service each hour for the latest data.

2. **Trigger Orders**: Stop/trigger orders are automatically filtered out as they shouldn't appear in the orderbook until triggered.

3. **Order Types**: The service tracks:
   - Open orders (added to orderbook)
   - Filled orders (removed from orderbook)
   - Canceled orders (removed from orderbook)

4. **Security**: Currently uses insecure gRPC channel. For production, implement TLS.

## Development

### Running Tests

```bash
# Test the orderbook service
python3 test_orderbook_client.py --port 50053

# Validate orderbook integrity
python3 validate_orderbook.py --port 50053
```

### Debug Logging

Enable detailed logging:

```bash
RUST_LOG=debug ./target/release/orderbook-service-realtime --grpc-port 50053
```

## Troubleshooting

1. **Negative spreads**: If you see bid > ask, check that trigger orders are being filtered properly.

2. **Stale prices**: The service reads from hourly files. Restart to get current hour's data.

3. **Connection issues**: Ensure the gRPC port is accessible and not blocked by firewall.

4. **High CPU usage**: Normal during high-volume periods. The service is processing hundreds of orders per second.

## License

Proprietary - All rights reserved