use crate::fast_orderbook::FastOrderbook;
use crate::oracle_client::OracleClient;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::interval;
use tracing::{debug, info, warn};

pub struct MarkPriceService {
    orderbooks: HashMap<u32, Arc<FastOrderbook>>,
    oracle_client: Arc<OracleClient>,
    update_interval: Duration,
    cache: Arc<RwLock<HashMap<u32, CachedMarkPrice>>>,
}

struct CachedMarkPrice {
    market_id: u32,
    symbol: String,
    hl_mark_price: crate::mark_price_v2::HLMarkPriceResult,
    calculated_at: Instant,
    calculation_version: u64,
}

#[derive(Clone, Debug)]
pub struct MarkPriceUpdateEvent {
    pub market_id: u32,
    pub symbol: String,
    pub timestamp: i64,
    pub hl_mark_price: crate::mark_price_v2::HLMarkPriceResult,
    pub calculation_version: u64,
}

impl MarkPriceService {
    pub fn new(
        orderbooks: HashMap<u32, Arc<FastOrderbook>>,
        oracle_client: Arc<OracleClient>,
        update_interval: Duration,
    ) -> Self {
        Self {
            orderbooks,
            oracle_client,
            update_interval,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start the mark price calculation service
    /// Returns a broadcast receiver for mark price updates
    pub async fn start(self) -> broadcast::Receiver<MarkPriceUpdateEvent> {
        let (tx, rx) = broadcast::channel(1000);
        
        let cache = self.cache.clone();
        let orderbooks = self.orderbooks.clone();
        let oracle_client = self.oracle_client.clone();
        let update_interval = self.update_interval;
        
        tokio::spawn(async move {
            let mut ticker = interval(update_interval);
            let mut calculation_version = 0u64;
            
            info!(
                "Starting mark price service with {} markets, update interval: {:?}",
                orderbooks.len(),
                update_interval
            );
            
            loop {
                ticker.tick().await;
                calculation_version += 1;
                
                let start = Instant::now();
                let mut updated_count = 0;
                
                // Get all oracle prices at once
                let oracle_prices = oracle_client.get_all_cached_prices().await;
                
                // Calculate mark prices for all markets
                for (market_id, orderbook) in &orderbooks {
                    // Skip if orderbook is empty
                    let (bids, asks) = orderbook.get_snapshot(1);
                    if bids.is_empty() || asks.is_empty() {
                        continue;
                    }
                    
                    // Update oracle price if available
                    if let Some(oracle_price) = oracle_prices.get(&orderbook.symbol) {
                        orderbook.set_oracle_price(*oracle_price);
                    }
                    
                    // Calculate Hyperliquid mark price
                    orderbook.calculate_hl_mark_price();
                    
                    if let Some(hl_mark_price) = orderbook.get_hl_mark_price() {
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as i64;
                        
                        // Update cache
                        {
                            let mut cache_write = cache.write();
                            cache_write.insert(*market_id, CachedMarkPrice {
                                market_id: *market_id,
                                symbol: orderbook.symbol.clone(),
                                hl_mark_price: hl_mark_price.clone(),
                                calculated_at: Instant::now(),
                                calculation_version,
                            });
                        }
                        
                        // Broadcast update
                        let update = MarkPriceUpdateEvent {
                            market_id: *market_id,
                            symbol: orderbook.symbol.clone(),
                            timestamp,
                            hl_mark_price,
                            calculation_version,
                        };
                        
                        if let Err(e) = tx.send(update) {
                            debug!("No active mark price subscribers: {}", e);
                        }
                        
                        updated_count += 1;
                    }
                }
                
                let elapsed = start.elapsed();
                if elapsed > Duration::from_millis(100) {
                    warn!(
                        "Mark price calculation took {:?} for {} markets",
                        elapsed, updated_count
                    );
                } else {
                    debug!(
                        "Updated {} mark prices in {:?}",
                        updated_count, elapsed
                    );
                }
            }
        });
        
        rx
    }

    /// Get cached mark price for a market
    pub fn get_cached_mark_price(&self, market_id: u32) -> Option<(crate::mark_price_v2::HLMarkPriceResult, Duration)> {
        let cache = self.cache.read();
        cache.get(&market_id).map(|cached| {
            let age = cached.calculated_at.elapsed();
            (cached.hl_mark_price.clone(), age)
        })
    }
    
    /// Get all cached mark prices
    pub fn get_all_cached_mark_prices(&self) -> HashMap<u32, (String, crate::mark_price_v2::HLMarkPriceResult, Duration)> {
        let cache = self.cache.read();
        cache.iter()
            .map(|(market_id, cached)| {
                let age = cached.calculated_at.elapsed();
                (*market_id, (cached.symbol.clone(), cached.hl_mark_price.clone(), age))
            })
            .collect()
    }
}