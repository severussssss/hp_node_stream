# Mark Price Decoupling Design

## Current Problem

Mark price is calculated synchronously on every L2 update:

```rust
// CURRENT BAD DESIGN - in grpc_server.rs
while let Ok(update) = rx.recv().await {
    // ... get orderbook snapshot ...
    
    // THIS SHOULD NOT BE HERE!
    orderbook.update_mark_price();
    let hl_mark_price_data = calculate_hl_mark_price(&orderbook);
    
    // ... send snapshot with mark price ...
}
```

## Proposed Architecture

### 1. Separate Mark Price Service

```rust
// mark_price_service.rs
pub struct MarkPriceService {
    orderbooks: HashMap<u32, Arc<FastOrderbook>>,
    oracle_client: Arc<OracleClient>,
    cex_price_feeds: Arc<CexPriceAggregator>,
    update_interval: Duration,
}

impl MarkPriceService {
    pub async fn start(self) -> broadcast::Receiver<MarkPriceUpdate> {
        let (tx, rx) = broadcast::channel(1000);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(self.update_interval);
            
            loop {
                interval.tick().await;
                
                // Calculate mark prices for all markets
                for (market_id, orderbook) in &self.orderbooks {
                    if let Some(mark_price) = self.calculate_mark_price(market_id).await {
                        let _ = tx.send(MarkPriceUpdate {
                            market_id: *market_id,
                            timestamp: Utc::now(),
                            mark_price,
                        });
                    }
                }
            }
        });
        
        rx
    }
}
```

### 2. Independent gRPC Endpoints

```proto
service OrderbookService {
    // L2 Data - High frequency, raw data
    rpc SubscribeOrderbook(SubscribeRequest) returns (stream OrderbookSnapshot);
    
    // Mark Price - Lower frequency, calculated data  
    rpc SubscribeMarkPrices(MarkPriceRequest) returns (stream MarkPriceUpdate);
    
    // One-off queries
    rpc GetMarkPrice(GetMarkPriceRequest) returns (MarkPriceResponse);
}

message OrderbookSnapshot {
    uint32 market_id = 1;
    string symbol = 2;
    repeated Level bids = 3;
    repeated Level asks = 4;
    uint64 sequence = 5;
    int64 timestamp = 6;
    // NO MARK PRICE HERE!
}

message MarkPriceUpdate {
    uint32 market_id = 1;
    HyperliquidMarkPrice hl_mark_price = 2;
    int64 timestamp = 3;
    uint32 calculation_version = 4; // Track calculation method changes
}
```

### 3. Optimal Update Frequencies

```rust
// Configuration
const L2_UPDATE_FREQUENCY: Duration = Duration::from_millis(10);    // 100 Hz
const MARK_PRICE_FREQUENCY: Duration = Duration::from_secs(1);      // 1 Hz
const ORACLE_PRICE_FREQUENCY: Duration = Duration::from_secs(3);    // 0.33 Hz

// Different services run at different rates
struct ServiceConfig {
    l2_streaming: StreamConfig {
        max_depth: 50,
        throttle_ms: None,  // Real-time
        compression: true,
    },
    mark_price: MarkPriceConfig {
        update_interval_ms: 1000,
        include_components: true,
        markets: vec!["BTC", "ETH", "SOL"], // High-value markets
    },
}
```

### 4. Client-Side Composition

```python
# Python client example
class HyperliquidClient:
    def __init__(self):
        self.channel = grpc.insecure_channel('localhost:50051')
        self.orderbook_stub = OrderbookServiceStub(self.channel)
        
    def subscribe_with_mark_price(self, market_id):
        """Compose L2 and mark price streams client-side"""
        
        # Subscribe to L2 data
        l2_stream = self.orderbook_stub.SubscribeOrderbook(
            SubscribeRequest(market_ids=[market_id])
        )
        
        # Separately subscribe to mark prices
        mp_stream = self.orderbook_stub.SubscribeMarkPrices(
            MarkPriceRequest(market_ids=[market_id])
        )
        
        # Client decides how to combine them
        latest_mark_price = None
        
        # Process streams independently
        # Update UI at different rates
        # Cache mark price between updates
```

### 5. Caching Layer

```rust
// Cache mark prices with TTL
pub struct MarkPriceCache {
    cache: DashMap<u32, CachedMarkPrice>,
}

struct CachedMarkPrice {
    value: HyperliquidMarkPrice,
    calculated_at: Instant,
    expires_at: Instant,
}

impl MarkPriceCache {
    pub fn get(&self, market_id: u32) -> Option<HyperliquidMarkPrice> {
        self.cache.get(&market_id)
            .filter(|entry| entry.expires_at > Instant::now())
            .map(|entry| entry.value.clone())
    }
}
```

## Benefits of Decoupling

1. **Performance**: L2 updates aren't blocked by mark price calculations
2. **Scalability**: Can scale L2 and mark price services independently  
3. **Flexibility**: Clients choose what data they need
4. **Efficiency**: Mark prices calculated at appropriate frequency (1Hz vs 100Hz)
5. **Caching**: Mark prices can be cached and shared across clients
6. **Cost**: Reduce compute by 99% (calculate 1x/sec instead of 1000x/sec)

## Migration Path

```rust
// Phase 1: Add separate mark price endpoint (backward compatible)
impl OrderbookService {
    async fn subscribe_orderbook_v2(&self, request) -> Result<Stream> {
        // New endpoint without mark price
    }
}

// Phase 2: Deprecate mark price in orderbook snapshots
message OrderbookSnapshot {
    // ...
    option deprecated = true;
    MarkPrice mark_price = 7 [deprecated = true];
}

// Phase 3: Remove mark price from L2 stream
// Clients must use separate endpoint
```

## Example Architecture

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   L2 Streamer   │     │ Mark Price Svc  │     │  Oracle Client  │
│   (Real-time)   │     │    (1 Hz)       │     │    (0.3 Hz)     │
└────────┬────────┘     └────────┬────────┘     └────────┬────────┘
         │                       │                         │
         ▼                       ▼                         ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Shared Orderbooks                         │
│                     (Arc<FastOrderbook>)                        │
└─────────────────────────────────────────────────────────────────┘
         ▲                       ▲                         ▲
         │                       │                         │
┌────────┴────────┐     ┌────────┴────────┐     ┌────────┴────────┐
│  gRPC L2 Stream │     │ gRPC Mark Price │     │   REST API      │
│   Endpoint      │     │   Endpoint      │     │   (Cached)      │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```