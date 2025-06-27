# Oracle Price Feed Integration Plan

## Current Situation
The Hyperliquid node infrastructure doesn't expose oracle prices directly through the local node data. Oracle prices are managed by validators and published on-chain.

## Options for Getting Oracle Prices

### 1. **Direct from Hyperliquid API** (Recommended)
Hyperliquid provides oracle prices through their public API endpoints:

```python
import requests

def get_oracle_prices():
    # Hyperliquid public API endpoint
    url = "https://api.hyperliquid.xyz/info"
    
    # Get meta info which includes oracle prices
    payload = {
        "type": "meta"
    }
    
    response = requests.post(url, json=payload)
    data = response.json()
    
    # Oracle prices are in the universe field
    oracle_prices = {}
    for asset in data.get('universe', []):
        symbol = asset['name']
        oracle_price = float(asset['markPx'])  # This is the oracle price
        oracle_prices[symbol] = oracle_price
    
    return oracle_prices
```

### 2. **WebSocket Subscription**
Subscribe to real-time oracle price updates:

```python
import websocket
import json

def subscribe_to_oracle_prices():
    ws_url = "wss://api.hyperliquid.xyz/ws"
    
    def on_message(ws, message):
        data = json.loads(message)
        if data.get('channel') == 'activeAssetCtx':
            # Extract oracle prices from the update
            for asset in data.get('data', {}).get('activeAssetCtx', []):
                oracle_price = asset.get('oracle')
                if oracle_price:
                    print(f"{asset['coin']}: ${oracle_price}")
    
    ws = websocket.WebSocketApp(ws_url, on_message=on_message)
    
    # Subscribe to asset context updates
    ws.on_open = lambda ws: ws.send(json.dumps({
        "method": "subscribe",
        "subscription": {"type": "activeAssetCtx"}
    }))
    
    ws.run_forever()
```

### 3. **On-Chain Data** (Advanced)
Oracle prices are published on-chain by validators. You could:
- Monitor validator transactions that update oracle prices
- Read from the smart contracts that store oracle data
- This requires deeper integration with the Hyperliquid blockchain

### 4. **CEX Price Feeds**
For the CEX component of mark price, connect to exchange APIs:

```python
# Example for Binance
async def get_binance_perp_price(symbol):
    url = f"https://fapi.binance.com/fapi/v1/ticker/price?symbol={symbol}USDT"
    response = await session.get(url)
    data = await response.json()
    return float(data['price'])

# Similar implementations for OKX, Bybit, Gate.io, MEXC
```

## Recommended Implementation Steps

1. **Start with API polling** for oracle prices (every 3 seconds as per Hyperliquid docs)
2. **Add CEX price feeds** using their REST APIs or WebSockets
3. **Implement price caching** to reduce API calls
4. **Add WebSocket support** for real-time updates
5. **Consider rate limiting** and fallback mechanisms

## Integration with Our Service

```rust
// In main_realtime.rs, add a price feed manager
pub struct PriceFeedManager {
    oracle_prices: Arc<RwLock<HashMap<String, f64>>>,
    cex_prices: Arc<RwLock<HashMap<String, CEXPrices>>>,
}

impl PriceFeedManager {
    pub async fn start_price_feeds(&self) {
        // Spawn tasks for each price source
        tokio::spawn(self.poll_hyperliquid_oracle());
        tokio::spawn(self.poll_cex_prices());
    }
    
    async fn poll_hyperliquid_oracle(&self) {
        loop {
            // Fetch oracle prices from API
            // Update orderbooks with new prices
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
}
```

## Quick Start Example

```python
# test_oracle_integration.py
import requests
import time

def test_oracle_prices():
    url = "https://api.hyperliquid.xyz/info"
    
    while True:
        try:
            response = requests.post(url, json={"type": "meta"})
            data = response.json()
            
            # Find BTC oracle price
            for asset in data.get('universe', []):
                if asset['name'] == 'BTC':
                    oracle_price = float(asset['markPx'])
                    print(f"BTC Oracle Price: ${oracle_price:,.2f}")
                    break
            
            time.sleep(3)  # Poll every 3 seconds
            
        except Exception as e:
            print(f"Error: {e}")
            time.sleep(5)

if __name__ == "__main__":
    test_oracle_prices()
```

## Next Steps

1. Test the API endpoints to verify oracle price availability
2. Implement a simple Python client to fetch prices
3. Add Rust integration to update orderbook oracle prices
4. Set up CEX WebSocket connections for real-time prices
5. Monitor and log price feed reliability

This approach gives us a clear path to getting the oracle prices needed for Hyperliquid's exact mark price calculation.