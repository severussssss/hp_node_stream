use crate::orderbook::{Orderbook, OrderbookStats};
use crate::types::{Order, OrderStatus, Side};
use anyhow::Result;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

pub const BTC_MARKET_ID: u32 = 0;
pub const HYPE_MARKET_ID: u32 = 159;

#[derive(Debug, Clone)]
pub struct OrderbookSnapshot {
    pub market_id: u32,
    pub symbol: String,
    pub timestamp: u64,
    pub sequence: u64,
    pub bids: Vec<(f64, f64)>, // (price, size)
    pub asks: Vec<(f64, f64)>, // (price, size)
}

pub struct MarketManager {
    markets: HashMap<u32, Arc<RwLock<Orderbook>>>,
    market_symbols: HashMap<u32, String>,
    update_tx: broadcast::Sender<OrderbookSnapshot>,
    order_count: AtomicU32,
    parse_error_count: AtomicU32,
}

impl MarketManager {
    pub fn new() -> (Self, broadcast::Receiver<OrderbookSnapshot>) {
        let (update_tx, update_rx) = broadcast::channel(10000);
        
        let mut market_symbols = HashMap::new();
        market_symbols.insert(BTC_MARKET_ID, "BTC-PERP".to_string());
        market_symbols.insert(HYPE_MARKET_ID, "HYPE-PERP".to_string());
        
        let manager = Self {
            markets: HashMap::new(),
            market_symbols,
            update_tx,
            order_count: AtomicU32::new(0),
            parse_error_count: AtomicU32::new(0),
        };
        
        (manager, update_rx)
    }
    
    pub fn initialize_markets(&mut self, market_ids: Vec<u32>) {
        for market_id in market_ids {
            let symbol = self.market_symbols.get(&market_id)
                .unwrap_or(&format!("MARKET-{}", market_id))
                .clone();
            
            let orderbook = Arc::new(RwLock::new(Orderbook::new(market_id, symbol.clone())));
            self.markets.insert(market_id, orderbook);
            
            info!("Initialized orderbook for market {} ({})", market_id, symbol);
        }
    }
    
    pub fn process_order_status(&self, market_id: u32, order_status: OrderStatus) -> Result<()> {
        if let Some(orderbook_arc) = self.markets.get(&market_id) {
            let mut orderbook = orderbook_arc.write();
            
            // Convert OrderStatus to Order
            let price = match order_status.limit_px.parse::<f64>() {
                Ok(p) => p,
                Err(e) => {
                    let error_count = self.parse_error_count.fetch_add(1, Ordering::Relaxed);
                    if error_count < 5 {
                        warn!("Failed to parse price '{}': {}", order_status.limit_px, e);
                    }
                    return Ok(());
                }
            };
            
            let size = match order_status.sz.parse::<f64>() {
                Ok(s) => s,
                Err(e) => {
                    let error_count = self.parse_error_count.fetch_add(1, Ordering::Relaxed);
                    if error_count < 5 {
                        warn!("Failed to parse size '{}': {}", order_status.sz, e);
                    }
                    return Ok(());
                }
            };
            
            let order = Order {
                id: order_status.oid,
                side: if order_status.is_buy { Side::Buy } else { Side::Sell },
                price,
                size,
                timestamp: order_status.timestamp,
            };
            
            // Log progress periodically
            let count = self.order_count.fetch_add(1, Ordering::Relaxed);
            if count < 5 || count % 100 == 0 {
                info!(
                    "Processed {} orders. Current order: market={}, id={}, price={}, size={}, side={:?}", 
                    count + 1, market_id, order.id, order.price, order.size, order.side
                );
            }
            
            // Process based on order size and status
            if order_status.is_cancelled || order_status.is_filled || order.size == 0.0 {
                // Cancel/remove order
                orderbook.cancel_order(order.id);
            } else {
                // Add or update order
                orderbook.update_order(order);
            }
            
            // Get snapshot and broadcast update
            let snapshot = self.create_snapshot(&orderbook, 50);
            
            // Log snapshot info periodically
            if count < 5 || count % 100 == 0 {
                info!(
                    "Orderbook snapshot for market {}: {} bids, {} asks", 
                    market_id, snapshot.bids.len(), snapshot.asks.len()
                );
                if !snapshot.bids.is_empty() {
                    info!("Best bid: {:?}", snapshot.bids[0]);
                }
                if !snapshot.asks.is_empty() {
                    info!("Best ask: {:?}", snapshot.asks[0]);
                }
            }
            
            drop(orderbook); // Release lock before broadcasting
            
            // Send update to subscribers
            let _ = self.update_tx.send(snapshot);
            
            Ok(())
        } else {
            warn!("Received order for untracked market: {}", market_id);
            Ok(())
        }
    }
    
    pub fn get_orderbook_snapshot(&self, market_id: u32, depth: usize) -> Option<OrderbookSnapshot> {
        self.markets.get(&market_id).map(|orderbook_arc| {
            let orderbook = orderbook_arc.read();
            self.create_snapshot(&orderbook, depth)
        })
    }
    
    fn create_snapshot(&self, orderbook: &Orderbook, depth: usize) -> OrderbookSnapshot {
        let (bids, asks) = orderbook.get_snapshot(depth);
        
        OrderbookSnapshot {
            market_id: orderbook.market_id,
            symbol: orderbook.symbol.clone(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            sequence: orderbook.sequence,
            bids,
            asks,
        }
    }
    
    pub fn get_market_info(&self) -> Vec<(u32, String)> {
        self.market_symbols.iter()
            .filter(|(id, _)| self.markets.contains_key(id))
            .map(|(id, symbol)| (*id, symbol.clone()))
            .collect()
    }
    
    pub fn get_market_stats(&self, market_id: u32) -> Option<OrderbookStats> {
        self.markets.get(&market_id).map(|orderbook_arc| {
            let orderbook = orderbook_arc.read();
            orderbook.get_stats()
        })
    }
}