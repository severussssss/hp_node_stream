use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use parking_lot::RwLock;
use smallvec::SmallVec;
use crate::mark_price::{MarkPriceCalculator, MarkPriceResult};
use crate::mark_price_v2::{HyperliquidMarkPriceCalculator, MarkPriceInputs, CEXPrices, MarkPriceResult as HLMarkPriceResult};

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
    
    // Mark price calculation (old version for compatibility)
    mark_price_calc: RwLock<MarkPriceCalculator>,
    last_mark_price: RwLock<Option<MarkPriceResult>>,
    
    // Hyperliquid's exact mark price calculation
    hl_mark_price_calc: RwLock<HyperliquidMarkPriceCalculator>,
    last_hl_mark_price: RwLock<Option<HLMarkPriceResult>>,
    
    // External price feeds (would come from oracle/CEX in production)
    oracle_price: RwLock<Option<f64>>,
    cex_prices: RwLock<Option<CEXPrices>>,
    last_trade_price: RwLock<Option<f64>>,
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
        // Extract base currency from TradableProduct format for impact notional
        let base_currency = if symbol.contains('/') {
            symbol.split('/').next().unwrap_or(&symbol).to_string()
        } else {
            symbol.clone()
        };
        
        // Configure mark price calculator with sensible defaults
        let impact_notional = match base_currency.as_str() {
            "BTC" => 50000.0,   // $50k impact for BTC
            "ETH" => 20000.0,   // $20k impact for ETH
            _ => 10000.0,       // $10k impact for others
        };
        
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
            mark_price_calc: RwLock::new(MarkPriceCalculator::new(
                impact_notional,
                10,  // 10 second EMA
                50.0 // 50 bps max deviation
            )),
            last_mark_price: RwLock::new(None),
            hl_mark_price_calc: RwLock::new(HyperliquidMarkPriceCalculator::new()),
            last_hl_mark_price: RwLock::new(None),
            oracle_price: RwLock::new(None),
            cex_prices: RwLock::new(None),
            last_trade_price: RwLock::new(None),
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
    
    pub fn update_mark_price(&self) -> Option<MarkPriceResult> {
        let bids = self.bid_levels.read();
        let asks = self.ask_levels.read();
        
        if bids.is_empty() || asks.is_empty() {
            return None;
        }
        
        // Get orderbook data for mark price calculation
        let bid_levels: Vec<(f64, f64)> = bids
            .iter()
            .take(20)  // Use top 20 levels for impact calculation
            .map(|level| (level.price, level.total_size))
            .collect();
            
        let ask_levels: Vec<(f64, f64)> = asks
            .iter()
            .take(20)
            .map(|level| (level.price, level.total_size))
            .collect();
        
        // Release read locks before taking write lock
        drop(bids);
        drop(asks);
        
        // Calculate new mark price
        let mut calc = self.mark_price_calc.write();
        let mark_price_result = calc.calculate_mark_price(&bid_levels, &ask_levels);
        
        // Store result
        if let Some(ref result) = mark_price_result {
            *self.last_mark_price.write() = Some(result.clone());
        }
        
        mark_price_result
    }
    
    pub fn get_mark_price(&self) -> Option<MarkPriceResult> {
        self.last_mark_price.read().clone()
    }
    
    pub fn get_mark_price_value(&self) -> Option<f64> {
        self.last_mark_price.read().as_ref().map(|r| r.mark_price)
    }
    
    // Hyperliquid's exact mark price calculation methods
    
    pub fn update_oracle_price(&self, oracle_price: f64) {
        *self.oracle_price.write() = Some(oracle_price);
        self.hl_mark_price_calc.write().update_oracle_price(oracle_price);
    }
    
    pub fn update_cex_prices(&self, cex_prices: CEXPrices) {
        *self.cex_prices.write() = Some(cex_prices);
    }
    
    pub fn update_last_trade(&self, trade_price: f64) {
        *self.last_trade_price.write() = Some(trade_price);
        self.hl_mark_price_calc.write().update_trade(trade_price);
    }
    
    pub fn calculate_hl_mark_price(&self) -> Option<HLMarkPriceResult> {
        let bids = self.bid_levels.read();
        let asks = self.ask_levels.read();
        
        if bids.is_empty() || asks.is_empty() {
            return None;
        }
        
        let best_bid = bids[0].price;
        let best_ask = asks[0].price;
        
        // Release read locks
        drop(bids);
        drop(asks);
        
        let inputs = MarkPriceInputs {
            best_bid,
            best_ask,
            last_trade: *self.last_trade_price.read(),
            oracle_price: *self.oracle_price.read(),
            cex_prices: self.cex_prices.read().clone(),
        };
        
        let mut calc = self.hl_mark_price_calc.write();
        let result = calc.calculate_mark_price(&inputs);
        
        *self.last_hl_mark_price.write() = Some(result.clone());
        
        Some(result)
    }
    
    pub fn get_hl_mark_price(&self) -> Option<HLMarkPriceResult> {
        self.last_hl_mark_price.read().clone()
    }
    
    pub fn get_hl_mark_price_value(&self) -> Option<f64> {
        self.last_hl_mark_price.read().as_ref().map(|r| r.mark_price)
    }
    
    pub fn get_oracle_price(&self) -> Option<f64> {
        *self.oracle_price.read()
    }
    
    pub fn get_last_trade_price(&self) -> Option<f64> {
        *self.last_trade_price.read()
    }
    
    pub fn get_cex_prices(&self) -> Option<CEXPrices> {
        self.cex_prices.read().clone()
    }
}