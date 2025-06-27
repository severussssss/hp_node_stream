use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Hyperliquid's exact mark price calculation methodology
/// 
/// Mark Price = Median of:
/// 1. Oracle price + 150s EMA(mid - oracle)
/// 2. Median(best_bid, best_ask, last_trade) on Hyperliquid
/// 3. Weighted median of CEX perp prices (Binance:3, OKX:2, Bybit:2, Gate:1, MEXC:1)
/// 
/// If only 2 inputs exist, add 30s EMA of Hyperliquid mid to median calculation
#[derive(Debug, Clone)]
pub struct HyperliquidMarkPriceCalculator {
    // EMA calculators
    oracle_basis_ema: EMACalculator,      // 150s EMA of (mid - oracle)
    fallback_mid_ema: EMACalculator,      // 30s EMA of Hyperliquid mid
    
    // Latest values
    last_oracle_price: Option<f64>,
    last_trade_price: Option<f64>,
    last_update: Instant,
}

#[derive(Debug, Clone)]
pub struct EMACalculator {
    time_constant_minutes: f64,
    numerator: f64,
    denominator: f64,
    last_update: Option<Instant>,
}

#[derive(Debug, Clone)]
pub struct CEXPrices {
    pub binance: Option<f64>,
    pub okx: Option<f64>,
    pub bybit: Option<f64>,
    pub gate: Option<f64>,
    pub mexc: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct MarkPriceInputs {
    pub best_bid: f64,
    pub best_ask: f64,
    pub last_trade: Option<f64>,
    pub oracle_price: Option<f64>,
    pub cex_prices: Option<CEXPrices>,
}

#[derive(Debug, Clone)]
pub struct MarkPriceResult {
    pub mark_price: f64,
    pub oracle_adjusted: Option<f64>,
    pub internal_median: f64,
    pub cex_median: Option<f64>,
    pub used_fallback: bool,
}

impl EMACalculator {
    pub fn new(time_constant_minutes: f64) -> Self {
        Self {
            time_constant_minutes,
            numerator: 0.0,
            denominator: 0.0,
            last_update: None,
        }
    }
    
    /// Update EMA using Hyperliquid's exact formula:
    /// numerator -> numerator * exp(-t / 2.5 minutes) + sample * t
    /// denominator -> denominator * exp(-t / 2.5 minutes) + t
    pub fn update(&mut self, sample: f64, now: Instant) -> f64 {
        if let Some(last) = self.last_update {
            let t = now.duration_since(last).as_secs_f64() / 60.0; // Convert to minutes
            let decay = (-t / self.time_constant_minutes).exp();
            
            self.numerator = self.numerator * decay + sample * t;
            self.denominator = self.denominator * decay + t;
        } else {
            // First sample
            self.numerator = sample;
            self.denominator = 1.0;
        }
        
        self.last_update = Some(now);
        
        if self.denominator > 0.0 {
            self.numerator / self.denominator
        } else {
            sample
        }
    }
    
    pub fn get_value(&self) -> Option<f64> {
        if self.denominator > 0.0 {
            Some(self.numerator / self.denominator)
        } else {
            None
        }
    }
}

impl HyperliquidMarkPriceCalculator {
    pub fn new() -> Self {
        Self {
            oracle_basis_ema: EMACalculator::new(2.5), // 150s = 2.5 minutes
            fallback_mid_ema: EMACalculator::new(0.5), // 30s = 0.5 minutes
            last_oracle_price: None,
            last_trade_price: None,
            last_update: Instant::now(),
        }
    }
    
    pub fn calculate_mark_price(&mut self, inputs: &MarkPriceInputs) -> MarkPriceResult {
        let now = Instant::now();
        let mid_price = (inputs.best_bid + inputs.best_ask) / 2.0;
        
        // Update last trade if provided
        if let Some(trade) = inputs.last_trade {
            self.last_trade_price = Some(trade);
        }
        
        // Update fallback EMA (30s EMA of mid)
        self.fallback_mid_ema.update(mid_price, now);
        
        let mut median_inputs = Vec::new();
        
        // Input 1: Oracle price + 150s EMA(mid - oracle)
        let oracle_adjusted = if let Some(oracle) = inputs.oracle_price {
            self.last_oracle_price = Some(oracle);
            
            // Update basis EMA
            let basis = mid_price - oracle;
            let ema_basis = self.oracle_basis_ema.update(basis, now);
            
            let adjusted = oracle + ema_basis;
            median_inputs.push(adjusted);
            Some(adjusted)
        } else {
            None
        };
        
        // Input 2: Median of internal book data
        let mut internal_prices = vec![inputs.best_bid, inputs.best_ask];
        if let Some(last_trade) = self.last_trade_price {
            internal_prices.push(last_trade);
        }
        let internal_median = calculate_median(&mut internal_prices);
        median_inputs.push(internal_median);
        
        // Input 3: Weighted median of CEX prices
        let cex_median = if let Some(ref cex) = inputs.cex_prices {
            if let Some(median) = calculate_cex_weighted_median(cex) {
                median_inputs.push(median);
                Some(median)
            } else {
                None
            }
        } else {
            None
        };
        
        // Fallback: If only 2 inputs exist, add 30s EMA of mid
        let used_fallback = if median_inputs.len() == 2 {
            if let Some(fallback) = self.fallback_mid_ema.get_value() {
                median_inputs.push(fallback);
                true
            } else {
                false
            }
        } else {
            false
        };
        
        // Calculate final mark price as median
        let mark_price = calculate_median(&mut median_inputs);
        
        self.last_update = now;
        
        MarkPriceResult {
            mark_price,
            oracle_adjusted,
            internal_median,
            cex_median,
            used_fallback,
        }
    }
    
    pub fn update_oracle_price(&mut self, oracle_price: f64) {
        self.last_oracle_price = Some(oracle_price);
    }
    
    pub fn update_trade(&mut self, trade_price: f64) {
        self.last_trade_price = Some(trade_price);
    }
}

/// Calculate median of a mutable vector (will be sorted)
fn calculate_median(values: &mut Vec<f64>) -> f64 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let len = values.len();
    
    if len == 0 {
        0.0
    } else if len % 2 == 0 {
        (values[len / 2 - 1] + values[len / 2]) / 2.0
    } else {
        values[len / 2]
    }
}

/// Calculate weighted median of CEX prices using Hyperliquid's weights:
/// Binance: 3, OKX: 2, Bybit: 2, Gate: 1, MEXC: 1
fn calculate_cex_weighted_median(cex: &CEXPrices) -> Option<f64> {
    let mut weighted_values = Vec::new();
    
    // Add values according to their weights
    if let Some(price) = cex.binance {
        for _ in 0..3 {
            weighted_values.push(price);
        }
    }
    if let Some(price) = cex.okx {
        for _ in 0..2 {
            weighted_values.push(price);
        }
    }
    if let Some(price) = cex.bybit {
        for _ in 0..2 {
            weighted_values.push(price);
        }
    }
    if let Some(price) = cex.gate {
        weighted_values.push(price);
    }
    if let Some(price) = cex.mexc {
        weighted_values.push(price);
    }
    
    if weighted_values.is_empty() {
        None
    } else {
        Some(calculate_median(&mut weighted_values))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_weighted_median() {
        let cex = CEXPrices {
            binance: Some(100.0),  // Weight 3
            okx: Some(101.0),      // Weight 2
            bybit: Some(102.0),    // Weight 2
            gate: Some(103.0),     // Weight 1
            mexc: Some(104.0),     // Weight 1
        };
        
        // Total 9 values: 100,100,100,101,101,102,102,103,104
        // Sorted: 100,100,100,101,101,102,102,103,104
        // Median is the 5th value (index 4) = 101
        let median = calculate_cex_weighted_median(&cex).unwrap();
        assert_eq!(median, 101.0);
    }
    
    #[test]
    fn test_ema_calculator() {
        let mut ema = EMACalculator::new(2.5);
        let now = Instant::now();
        
        // First update
        let val1 = ema.update(100.0, now);
        assert_eq!(val1, 100.0);
        
        // Second update after 1 minute
        let later = now + Duration::from_secs(60);
        let val2 = ema.update(110.0, later);
        
        // Should be between 100 and 110, closer to 110 due to time decay
        assert!(val2 > 100.0 && val2 < 110.0);
    }
}