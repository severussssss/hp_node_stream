use crate::types::{Order, OrderLocation, Side};
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone)]
pub struct PriceLevel {
    pub price: f64,
    pub orders: Vec<Order>,
}

pub struct Orderbook {
    pub symbol: String,
    pub market_id: u32,
    bids: BTreeMap<OrderedFloat<f64>, Vec<Order>>,
    asks: BTreeMap<OrderedFloat<f64>, Vec<Order>>,
    orders: HashMap<u64, OrderLocation>,
    pub sequence: u64,
    max_orders_per_level: usize,
    max_levels_per_side: usize,
    max_total_orders: usize,
}

impl Orderbook {
    pub fn new(market_id: u32, symbol: String) -> Self {
        Self {
            symbol,
            market_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
            sequence: 0,
            max_orders_per_level: 100,
            max_levels_per_side: 1000,
            max_total_orders: 10000,
        }
    }
    
    pub fn with_limits(market_id: u32, symbol: String, max_orders_per_level: usize, max_levels_per_side: usize, max_total_orders: usize) -> Self {
        Self {
            symbol,
            market_id,
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: HashMap::new(),
            sequence: 0,
            max_orders_per_level,
            max_levels_per_side,
            max_total_orders,
        }
    }

    pub fn add_order(&mut self, order: Order) {
        // Check if we've hit the total order limit
        if self.orders.len() >= self.max_total_orders {
            // Skip adding new orders to prevent OOM
            return;
        }
        
        self.sequence += 1;
        
        let (side, price_key) = if order.side == Side::Buy {
            (Side::Buy, OrderedFloat(-order.price))
        } else {
            (Side::Sell, OrderedFloat(order.price))
        };

        let book = if order.side == Side::Buy {
            &mut self.bids
        } else {
            &mut self.asks
        };
        
        // Check if we have too many price levels
        if book.len() >= self.max_levels_per_side && !book.contains_key(&price_key) {
            // Remove the worst price level to make room
            if order.side == Side::Buy {
                // For bids, remove the lowest bid (highest negative value)
                if let Some(&worst_price) = book.keys().last() {
                    book.remove(&worst_price);
                }
            } else {
                // For asks, remove the highest ask
                if let Some(&worst_price) = book.keys().last() {
                    book.remove(&worst_price);
                }
            }
        }

        let orders = book.entry(price_key).or_insert_with(Vec::new);
        
        // Check if this price level has too many orders
        if orders.len() >= self.max_orders_per_level {
            // Remove oldest order at this level
            if let Some(old_order) = orders.first() {
                self.orders.remove(&old_order.id);
            }
            orders.remove(0);
        }
        
        let index = orders.len();
        let order_id = order.id;
        orders.push(order.clone());

        self.orders.insert(
            order_id,
            OrderLocation {
                side,
                price: order.price,
                index,
            },
        );
    }

    pub fn cancel_order(&mut self, order_id: u64) {
        self.sequence += 1;
        
        if let Some(location) = self.orders.remove(&order_id) {
            let book = if location.side == Side::Buy {
                &mut self.bids
            } else {
                &mut self.asks
            };

            let price_key = if location.side == Side::Buy {
                OrderedFloat(-location.price)
            } else {
                OrderedFloat(location.price)
            };

            if let Some(orders) = book.get_mut(&price_key) {
                if location.index < orders.len() {
                    orders.swap_remove(location.index);
                    if orders.is_empty() {
                        book.remove(&price_key);
                    }
                }
            }
        }
    }

    pub fn update_order(&mut self, order: Order) {
        // First cancel the old order if it exists
        if self.orders.contains_key(&order.id) {
            self.cancel_order(order.id);
        }
        // Then add the updated order
        self.add_order(order);
    }

    pub fn get_snapshot(&self, depth: usize) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
        let mut bid_levels = Vec::new();
        for (price_key, orders) in self.bids.iter().take(if depth == 0 { usize::MAX } else { depth }) {
            let price = -price_key.0;
            let quantity: f64 = orders.iter().map(|o| o.size).sum();
            bid_levels.push((price, quantity));
        }

        let mut ask_levels = Vec::new();
        for (price_key, orders) in self.asks.iter().take(if depth == 0 { usize::MAX } else { depth }) {
            let price = price_key.0;
            let quantity: f64 = orders.iter().map(|o| o.size).sum();
            ask_levels.push((price, quantity));
        }

        (bid_levels, ask_levels)
    }

    pub fn clear(&mut self) {
        self.bids.clear();
        self.asks.clear();
        self.orders.clear();
        self.sequence = 0;
    }
    
    pub fn get_stats(&self) -> OrderbookStats {
        OrderbookStats {
            total_orders: self.orders.len(),
            bid_levels: self.bids.len(),
            ask_levels: self.asks.len(),
            max_orders_reached: self.orders.len() >= self.max_total_orders,
        }
    }
}

#[derive(Debug)]
pub struct OrderbookStats {
    pub total_orders: usize,
    pub bid_levels: usize,
    pub ask_levels: usize,
    pub max_orders_reached: bool,
}