use std::collections::HashMap;
use std::sync::RwLock;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopOrder {
    pub id: u64,
    pub user: String,
    pub coin: String,
    pub side: String,  // "B" or "A"
    pub price: f64,
    pub size: f64,
    pub trigger_condition: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone)]
pub struct RankedStopOrder {
    pub order: StopOrder,
    pub distance_to_trigger_bps: f64,
    pub expected_slippage_bps: f64,
    pub risk_score: f64,
    pub notional_value: f64,
}

pub struct StopOrderManager {
    // Market ID -> User -> Vec<StopOrder>
    orders_by_market: RwLock<HashMap<u32, HashMap<String, Vec<StopOrder>>>>,
    // Global list of all stop orders
    all_orders: RwLock<HashMap<u64, StopOrder>>,
}

impl StopOrderManager {
    pub fn new() -> Self {
        Self {
            orders_by_market: RwLock::new(HashMap::new()),
            all_orders: RwLock::new(HashMap::new()),
        }
    }

    pub fn add_stop_order(&self, market_id: u32, order: StopOrder) {
        let mut orders_by_market = self.orders_by_market.write().unwrap();
        let mut all_orders = self.all_orders.write().unwrap();
        
        // Add to global list
        all_orders.insert(order.id, order.clone());
        
        // Add to market/user map
        let market_orders = orders_by_market.entry(market_id).or_insert_with(HashMap::new);
        let user_orders = market_orders.entry(order.user.clone()).or_insert_with(Vec::new);
        user_orders.push(order);
    }

    pub fn remove_stop_order(&self, order_id: u64) {
        let mut all_orders = self.all_orders.write().unwrap();
        
        if let Some(order) = all_orders.remove(&order_id) {
            let mut orders_by_market = self.orders_by_market.write().unwrap();
            
            // Find and remove from market/user map
            for (_, market_orders) in orders_by_market.iter_mut() {
                if let Some(user_orders) = market_orders.get_mut(&order.user) {
                    user_orders.retain(|o| o.id != order_id);
                    if user_orders.is_empty() {
                        market_orders.remove(&order.user);
                    }
                    break;
                }
            }
        }
    }

    pub fn get_stop_orders_by_market(&self, market_id: u32) -> Vec<StopOrder> {
        let orders_by_market = self.orders_by_market.read().unwrap();
        
        if let Some(market_orders) = orders_by_market.get(&market_id) {
            market_orders
                .values()
                .flat_map(|user_orders| user_orders.iter().cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_stop_orders_by_user(&self, user: &str) -> Vec<StopOrder> {
        let orders_by_market = self.orders_by_market.read().unwrap();
        let mut result = Vec::new();
        
        for market_orders in orders_by_market.values() {
            if let Some(user_orders) = market_orders.get(user) {
                result.extend(user_orders.iter().cloned());
            }
        }
        
        result
    }

    pub fn get_all_stop_orders(&self) -> Vec<StopOrder> {
        let all_orders = self.all_orders.read().unwrap();
        all_orders.values().cloned().collect()
    }

    pub fn get_stop_order_count(&self) -> usize {
        self.all_orders.read().unwrap().len()
    }
    
    pub fn get_market_id_for_coin(&self, coin: &str) -> Option<u32> {
        crate::markets::get_market_id(coin)
    }

    pub fn calculate_slippage(
        &self,
        order: &StopOrder,
        orderbook_levels: &[(f64, f64)], // (price, size) pairs
        is_buy: bool,
    ) -> f64 {
        let mut remaining_size = order.size;
        let mut total_cost = 0.0;
        let mut filled_size = 0.0;

        for (price, size) in orderbook_levels {
            if remaining_size <= 0.0 {
                break;
            }

            let fill_size = remaining_size.min(*size);
            total_cost += fill_size * price;
            filled_size += fill_size;
            remaining_size -= fill_size;
        }

        if filled_size > 0.0 {
            let avg_fill_price = total_cost / filled_size;
            ((avg_fill_price - order.price).abs() / order.price) * 10000.0 // Return slippage in bps
        } else {
            1000.0 // Return 10% slippage if can't fill
        }
    }

    pub fn rank_stop_orders(
        &self,
        orders: Vec<StopOrder>,
        mid_prices: &HashMap<u32, f64>,
        orderbooks: &HashMap<u32, (Vec<(f64, f64)>, Vec<(f64, f64)>)>, // market_id -> (bids, asks)
        distance_weight: f64,
        slippage_weight: f64,
    ) -> Vec<RankedStopOrder> {
        let mut ranked_orders = Vec::new();

        for order in orders {
            if let Some(market_id) = self.get_market_id_for_coin(&order.coin) {
                if let (Some(mid_price), Some(book)) = (mid_prices.get(&market_id), orderbooks.get(&market_id)) {
                    let is_buy = order.side == "B";
                    let is_stop_loss = (is_buy && order.price > *mid_price) || (!is_buy && order.price < *mid_price);
                    
                    // Calculate distance to trigger
                    let distance_to_trigger_bps = if is_stop_loss {
                        if is_buy {
                            ((order.price - mid_price) / mid_price) * 10000.0
                        } else {
                            ((mid_price - order.price) / mid_price) * 10000.0
                        }
                    } else {
                        // Take profit orders
                        if is_buy {
                            ((mid_price - order.price) / mid_price) * 10000.0
                        } else {
                            ((order.price - mid_price) / mid_price) * 10000.0
                        }
                    };

                    // Calculate expected slippage
                    let orderbook_levels = if is_buy { &book.1 } else { &book.0 }; // Buy from asks, sell to bids
                    let expected_slippage_bps = self.calculate_slippage(&order, orderbook_levels, is_buy);

                    // Calculate risk score (0-100, higher = higher risk)
                    let distance_score = (100.0 - distance_to_trigger_bps.min(100.0)).max(0.0);
                    let slippage_score = expected_slippage_bps.min(100.0);
                    let risk_score = distance_weight * distance_score + slippage_weight * slippage_score;

                    let notional_value = order.price * order.size;

                    ranked_orders.push(RankedStopOrder {
                        order,
                        distance_to_trigger_bps,
                        expected_slippage_bps,
                        risk_score,
                        notional_value,
                    });
                }
            }
        }

        // Sort by risk score (descending - highest risk first)
        ranked_orders.sort_by(|a, b| b.risk_score.partial_cmp(&a.risk_score).unwrap());
        ranked_orders
    }
}