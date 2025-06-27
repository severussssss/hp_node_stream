use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct MarkPriceCalculator {
    // Configuration
    impact_notional: f64,     // Notional value for impact price calculation (e.g., $10,000)
    ema_period: Duration,     // Period for exponential moving average
    max_deviation_bps: f64,   // Maximum deviation from mid in basis points
    
    // State
    price_history: VecDeque<(Instant, f64)>,
    last_ema: Option<f64>,
    last_update: Instant,
}

#[derive(Debug, Clone)]
pub struct MarkPriceResult {
    pub mid_price: f64,
    pub impact_bid_price: f64,
    pub impact_ask_price: f64,
    pub impact_mid_price: f64,
    pub ema_price: f64,
    pub mark_price: f64,  // Final mark price
    pub confidence: f64,   // 0-1, based on orderbook depth
}

impl MarkPriceCalculator {
    pub fn new(impact_notional: f64, ema_period_secs: u64, max_deviation_bps: f64) -> Self {
        Self {
            impact_notional,
            ema_period: Duration::from_secs(ema_period_secs),
            max_deviation_bps,
            price_history: VecDeque::with_capacity(1000),
            last_ema: None,
            last_update: Instant::now(),
        }
    }
    
    pub fn calculate_mark_price(
        &mut self,
        bids: &[(f64, f64)],  // (price, size)
        asks: &[(f64, f64)],
    ) -> Option<MarkPriceResult> {
        // Need at least one bid and ask
        if bids.is_empty() || asks.is_empty() {
            return None;
        }
        
        let now = Instant::now();
        
        // 1. Calculate simple mid price
        let best_bid = bids[0].0;
        let best_ask = asks[0].0;
        let mid_price = (best_bid + best_ask) / 2.0;
        
        // 2. Calculate impact prices
        let impact_bid_price = self.calculate_impact_price(asks, self.impact_notional, true);
        let impact_ask_price = self.calculate_impact_price(bids, self.impact_notional, false);
        let impact_mid_price = (impact_bid_price + impact_ask_price) / 2.0;
        
        // 3. Calculate orderbook confidence (based on depth)
        let confidence = self.calculate_confidence(bids, asks);
        
        // 4. Update EMA
        self.update_price_history(now, impact_mid_price);
        let ema_price = self.calculate_ema(now);
        
        // 5. Calculate final mark price with safety checks
        let mark_price = self.calculate_final_mark_price(
            mid_price,
            impact_mid_price,
            ema_price,
            confidence,
        );
        
        self.last_update = now;
        
        Some(MarkPriceResult {
            mid_price,
            impact_bid_price,
            impact_ask_price,
            impact_mid_price,
            ema_price,
            mark_price,
            confidence,
        })
    }
    
    fn calculate_impact_price(
        &self,
        levels: &[(f64, f64)],
        target_notional: f64,
        is_buy: bool,
    ) -> f64 {
        let mut remaining_notional = target_notional;
        let mut total_cost = 0.0;
        let mut total_size = 0.0;
        
        for (price, size) in levels {
            if remaining_notional <= 0.0 {
                break;
            }
            
            let level_notional = price * size;
            let fill_notional = remaining_notional.min(level_notional);
            let fill_size = fill_notional / price;
            
            total_cost += fill_notional;
            total_size += fill_size;
            remaining_notional -= fill_notional;
        }
        
        if total_size > 0.0 {
            total_cost / total_size
        } else {
            // If we can't fill the impact size, use the last level price with penalty
            if let Some((price, _)) = levels.last() {
                if is_buy {
                    price * 1.01  // 1% penalty for insufficient liquidity
                } else {
                    price * 0.99
                }
            } else {
                0.0
            }
        }
    }
    
    fn calculate_confidence(&self, bids: &[(f64, f64)], asks: &[(f64, f64)]) -> f64 {
        // Calculate total notional value in top 5 levels
        let bid_notional: f64 = bids.iter()
            .take(5)
            .map(|(price, size)| price * size)
            .sum();
            
        let ask_notional: f64 = asks.iter()
            .take(5)
            .map(|(price, size)| price * size)
            .sum();
        
        let total_notional = bid_notional + ask_notional;
        let min_expected = self.impact_notional * 2.0;  // Expect at least 2x impact size
        
        // Confidence based on depth and balance
        let depth_confidence = (total_notional / min_expected).min(1.0);
        let balance_ratio = bid_notional.min(ask_notional) / bid_notional.max(ask_notional);
        
        depth_confidence * balance_ratio
    }
    
    fn update_price_history(&mut self, now: Instant, price: f64) {
        // Add new price
        self.price_history.push_back((now, price));
        
        // Remove old prices outside EMA period
        let cutoff = now - self.ema_period;
        while let Some((time, _)) = self.price_history.front() {
            if *time < cutoff {
                self.price_history.pop_front();
            } else {
                break;
            }
        }
    }
    
    fn calculate_ema(&mut self, _now: Instant) -> f64 {
        if self.price_history.is_empty() {
            return 0.0;
        }
        
        // Simple EMA calculation
        let alpha = 2.0 / (self.ema_period.as_secs_f64() + 1.0);
        
        if let Some(last_ema) = self.last_ema {
            // Update EMA with latest price
            if let Some((_, latest_price)) = self.price_history.back() {
                let new_ema = latest_price * alpha + last_ema * (1.0 - alpha);
                self.last_ema = Some(new_ema);
                new_ema
            } else {
                last_ema
            }
        } else {
            // Initialize EMA with simple average
            let sum: f64 = self.price_history.iter().map(|(_, p)| p).sum();
            let avg = sum / self.price_history.len() as f64;
            self.last_ema = Some(avg);
            avg
        }
    }
    
    fn calculate_final_mark_price(
        &self,
        mid_price: f64,
        impact_mid_price: f64,
        ema_price: f64,
        confidence: f64,
    ) -> f64 {
        // Weight components based on confidence
        let impact_weight = confidence * 0.4;
        let ema_weight = confidence * 0.4;
        let mid_weight = 1.0 - impact_weight - ema_weight;
        
        let weighted_price = mid_price * mid_weight + 
                           impact_mid_price * impact_weight + 
                           ema_price * ema_weight;
        
        // Apply deviation limits
        let max_deviation = mid_price * self.max_deviation_bps / 10000.0;
        let clamped_price = weighted_price.max(mid_price - max_deviation)
                                         .min(mid_price + max_deviation);
        
        clamped_price
    }
    
    pub fn get_last_mark_price(&self) -> Option<f64> {
        self.last_ema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_impact_price_calculation() {
        let calc = MarkPriceCalculator::new(10000.0, 10, 50.0);
        
        // Test orderbook
        let asks = vec![
            (100.0, 10.0),   // $1,000 notional
            (100.1, 50.0),   // $5,005 notional
            (100.2, 100.0),  // $10,020 notional
        ];
        
        // Should fill: 10 @ 100, 50 @ 100.1, 39.92 @ 100.2
        // Total: 10*100 + 50*100.1 + 39.92*100.2 = 1000 + 5005 + 3995.98 ≈ 10000
        let impact_price = calc.calculate_impact_price(&asks, 10000.0, true);
        assert!((impact_price - 100.096).abs() < 0.01);
    }
}