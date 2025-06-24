# Orderbook Service Deployment Guide

## Overview

The Rust orderbook service is now running on this EC2 instance at `localhost:50051`. It reconstructs L2 orderbooks from Hyperliquid order status data for BTC (ID: 0) and HYPE (ID: 159) perpetual markets.

## Current Status

✅ Service is built and running on port 50051
✅ Monitoring for order status files at `/home/ubuntu/node/hl/data/order_statuses/`
✅ gRPC endpoints are accessible
⏳ Waiting for order status data from Hyperliquid node

## Accessing from Your Strategy EC2

### 1. Security Group Configuration

Add inbound rule to this EC2's security group:
- Type: Custom TCP
- Port: 50051
- Source: Your strategy EC2's security group or private IP

### 2. Python Client Example

On your strategy EC2, install dependencies:
```bash
pip install grpcio protobuf
```

Copy the protobuf file and generate Python code:
```bash
# Copy orderbook.proto from this server
scp ubuntu@<this-ec2-ip>:/home/ubuntu/node/orderbook-service/proto/orderbook.proto .

# Generate Python code
python -m grpc_tools.protoc -I. --python_out=. --grpc_python_out=. orderbook.proto
```

### 3. Subscribe to L2 Orderbook Stream

```python
import asyncio
import grpc
import orderbook_pb2
import orderbook_pb2_grpc

async def subscribe_to_orderbooks():
    # Replace with this EC2's private IP
    server_address = "<ec2-private-ip>:50051"
    
    async with grpc.aio.insecure_channel(server_address) as channel:
        stub = orderbook_pb2_grpc.OrderbookServiceStub(channel)
        
        # Subscribe to BTC and HYPE orderbooks
        request = orderbook_pb2.SubscribeRequest(market_ids=[0, 159])
        
        async for snapshot in stub.SubscribeOrderbook(request):
            # Process L2 orderbook data
            if snapshot.market_id == 0:  # BTC
                process_btc_orderbook(snapshot)
            elif snapshot.market_id == 159:  # HYPE
                process_hype_orderbook(snapshot)

def process_btc_orderbook(snapshot):
    if snapshot.bids and snapshot.asks:
        best_bid = snapshot.bids[0]
        best_ask = snapshot.asks[0]
        spread = best_ask.price - best_bid.price
        
        print(f"BTC: Bid ${best_bid.price:.2f} x {best_bid.quantity:.4f}, "
              f"Ask ${best_ask.price:.2f} x {best_ask.quantity:.4f}, "
              f"Spread ${spread:.2f}")

def process_hype_orderbook(snapshot):
    # Similar processing for HYPE
    pass

if __name__ == "__main__":
    asyncio.run(subscribe_to_orderbooks())
```

## Service Management

### Start Service
```bash
cd /home/ubuntu/node/orderbook-service
./target/release/orderbook-service \
  --data-dir /home/ubuntu/node/hl/data \
  --grpc-port 50051 \
  --markets 0,159
```

### Check Status
```bash
# Check if running
ps aux | grep orderbook-service

# View logs
tail -f service.log

# Test connectivity
netstat -tlnp | grep 50051
```

### Using systemd (Recommended)
```bash
# Start service
sudo systemctl start orderbook-service

# Check status
sudo systemctl status orderbook-service

# View logs
sudo journalctl -u orderbook-service -f
```

## Important Notes

1. **Order Status Data**: The Hyperliquid node needs to be fully synced and writing order status files. This may take time after node restart.

2. **Performance**: The service maintains in-memory orderbooks with < 100μs latency from file update to gRPC delivery.

3. **Markets**: Currently configured for:
   - Market 0: BTC-PERP
   - Market 159: HYPE-PERP

4. **Data Flow**:
   ```
   Hyperliquid Node → Order Status Files → File Monitor → 
   Orderbook Engine → gRPC Stream → Your Strategy
   ```

## Troubleshooting

1. **No orderbook updates**:
   - Check if Hyperliquid node is synced: `docker logs hyperliquid-node-1`
   - Verify order status directory exists and has files
   - Check service logs for errors

2. **Connection refused**:
   - Verify service is running: `ps aux | grep orderbook-service`
   - Check firewall/security groups
   - Ensure using correct IP and port

3. **High latency**:
   - Monitor CPU/memory usage
   - Check network latency between EC2s
   - Consider using same availability zone

## Next Steps

1. Once order status data starts flowing, you'll receive real-time L2 orderbook updates
2. The service automatically handles order additions, modifications, and cancellations
3. Each update includes full orderbook snapshot with configurable depth
4. Updates are sent only when orderbook changes occur