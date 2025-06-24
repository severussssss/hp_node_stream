use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use smallvec::SmallVec;

const MAX_PRICE_LEVELS: usize = 1000;
const ORDERS_PER_LEVEL: usize = 8;

#[derive(Debug, Clone, Copy)]
pub struct Order {
    pub id: u64,
    pub price: f64,
    pub size: f64,
    pub timestamp: u64,
}

#[derive(Debug)]
pub struct PriceLevel {
    pub price: f64,
    pub total_size: f64,
    pub orders: SmallVec<[Order; ORDERS_PER_LEVEL]>,
}

impl PriceLevel {
    fn new(price: f64) -> Self {
        Self {
            price,
            total_size: 0.0,
            orders: SmallVec::new(),
        }
    }
    
    fn add_order(&mut self, order: Order) {
        self.orders.push(order);
        self.total_size += order.size;
    }
    
    fn remove_order(&mut self, order_id: u64) -> bool {
        if let Some(pos) = self.orders.iter().position(|o| o.id == order_id) {
            let order = self.orders.remove(pos);
            self.total_size -= order.size;
            true
        } else {
            false
        }
    }
}

pub struct FastOrderbook {
    pub market_id: u32,
    pub symbol: String,
    
    // Pre-allocated arrays for price levels
    bid_levels: RwLock<Vec<PriceLevel>>,
    ask_levels: RwLock<Vec<PriceLevel>>,
    
    // Atomic counters for lock-free stats
    pub sequence: AtomicU64,
    pub bid_count: AtomicUsize,
    pub ask_count: AtomicUsize,
    pub total_orders: AtomicUsize,
    
    // Delta tracking
    pub last_update_seq: AtomicU64,
}

#[derive(Debug, Clone)]
pub enum OrderbookDelta {
    AddBid { price: f64, size: f64, order_id: u64 },
    AddAsk { price: f64, size: f64, order_id: u64 },
    RemoveBid { price: f64, order_id: u64 },
    RemoveAsk { price: f64, order_id: u64 },
    Clear,
}

impl FastOrderbook {
    pub fn new(market_id: u32, symbol: String) -> Self {
        Self {
            market_id,
            symbol,
            bid_levels: RwLock::new(Vec::with_capacity(MAX_PRICE_LEVELS)),
            ask_levels: RwLock::new(Vec::with_capacity(MAX_PRICE_LEVELS)),
            sequence: AtomicU64::new(0),
            bid_count: AtomicUsize::new(0),
            ask_count: AtomicUsize::new(0),
            total_orders: AtomicUsize::new(0),
            last_update_seq: AtomicU64::new(0),
        }
    }
    
    pub fn add_order(&self, order: Order, is_buy: bool) -> OrderbookDelta {
        self.sequence.fetch_add(1, Ordering::Relaxed);
        self.total_orders.fetch_add(1, Ordering::Relaxed);
        
        if is_buy {
            let mut bids = self.bid_levels.write();
            
            // Find or create price level
            let pos = bids.binary_search_by(|level| {
                level.price.partial_cmp(&order.price).unwrap().reverse()
            });
            
            match pos {
                Ok(idx) => {
                    bids[idx].add_order(order);
                }
                Err(idx) => {
                    let mut level = PriceLevel::new(order.price);
                    level.add_order(order);
                    bids.insert(idx, level);
                    self.bid_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            
            OrderbookDelta::AddBid {
                price: order.price,
                size: order.size,
                order_id: order.id,
            }
        } else {
            let mut asks = self.ask_levels.write();
            
            // Find or create price level
            let pos = asks.binary_search_by(|level| {
                level.price.partial_cmp(&order.price).unwrap()
            });
            
            match pos {
                Ok(idx) => {
                    asks[idx].add_order(order);
                }
                Err(idx) => {
                    let mut level = PriceLevel::new(order.price);
                    level.add_order(order);
                    asks.insert(idx, level);
                    self.ask_count.fetch_add(1, Ordering::Relaxed);
                }
            }
            
            OrderbookDelta::AddAsk {
                price: order.price,
                size: order.size,
                order_id: order.id,
            }
        }
    }
    
    pub fn remove_order(&self, order_id: u64, price: f64, is_buy: bool) -> Option<OrderbookDelta> {
        self.sequence.fetch_add(1, Ordering::Relaxed);
        
        if is_buy {
            let mut bids = self.bid_levels.write();
            
            if let Ok(idx) = bids.binary_search_by(|level| {
                level.price.partial_cmp(&price).unwrap().reverse()
            }) {
                if bids[idx].remove_order(order_id) {
                    self.total_orders.fetch_sub(1, Ordering::Relaxed);
                    
                    // Remove empty level
                    if bids[idx].orders.is_empty() {
                        bids.remove(idx);
                        self.bid_count.fetch_sub(1, Ordering::Relaxed);
                    }
                    
                    return Some(OrderbookDelta::RemoveBid { price, order_id });
                }
            }
        } else {
            let mut asks = self.ask_levels.write();
            
            if let Ok(idx) = asks.binary_search_by(|level| {
                level.price.partial_cmp(&price).unwrap()
            }) {
                if asks[idx].remove_order(order_id) {
                    self.total_orders.fetch_sub(1, Ordering::Relaxed);
                    
                    // Remove empty level
                    if asks[idx].orders.is_empty() {
                        asks.remove(idx);
                        self.ask_count.fetch_sub(1, Ordering::Relaxed);
                    }
                    
                    return Some(OrderbookDelta::RemoveAsk { price, order_id });
                }
            }
        }
        
        None
    }
    
    pub fn get_snapshot(&self, depth: usize) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let bids = self.bid_levels.read();
        let asks = self.ask_levels.read();
        
        let bid_snapshot: Vec<_> = bids
            .iter()
            .take(depth)
            .map(|level| (level.price, level.total_size))
            .collect();
            
        let ask_snapshot: Vec<_> = asks
            .iter()
            .take(depth)
            .map(|level| (level.price, level.total_size))
            .collect();
            
        (bid_snapshot, ask_snapshot)
    }
    
    pub fn get_best_bid_ask(&self) -> Option<(f64, f64)> {
        let bids = self.bid_levels.read();
        let asks = self.ask_levels.read();
        
        match (bids.first(), asks.first()) {
            (Some(bid), Some(ask)) => Some((bid.price, ask.price)),
            _ => None,
        }
    }
    
    pub fn clear(&self) {
        self.bid_levels.write().clear();
        self.ask_levels.write().clear();
        self.bid_count.store(0, Ordering::Relaxed);
        self.ask_count.store(0, Ordering::Relaxed);
        self.total_orders.store(0, Ordering::Relaxed);
        self.sequence.fetch_add(1, Ordering::Relaxed);
    }
}