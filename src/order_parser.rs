use anyhow::{bail, Result};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Structured order message matching Hyperliquid's format
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderMessage {
    pub order: RawOrder,
    pub status: String,  // Keep as string to handle unknown statuses
    pub user: String,
    #[serde(default)]
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawOrder {
    pub oid: u64,
    pub coin: String,
    pub side: String,  // "B" or "A"
    
    #[serde(rename = "limitPx", deserialize_with = "deserialize_price")]
    pub limit_px: f64,
    
    #[serde(deserialize_with = "deserialize_size")]
    pub sz: f64,
    
    #[serde(default)]
    pub is_trigger: bool,
    
    #[serde(default, rename = "triggerCondition")]
    pub trigger_condition: String,
    
    pub timestamp: u64,
}

/// Deserialize price from either string or number
fn deserialize_price<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(s) => s
            .parse::<f64>()
            .map_err(|e| serde::de::Error::custom(format!("Invalid price string: {}", e))),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("Invalid price number")),
        v => Err(serde::de::Error::custom(format!(
            "Price must be string or number, got: {:?}",
            v
        ))),
    }
}

/// Deserialize size from either string or number
fn deserialize_size<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(s) => s
            .parse::<f64>()
            .map_err(|e| serde::de::Error::custom(format!("Invalid size string: {}", e))),
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| serde::de::Error::custom("Invalid size number")),
        v => Err(serde::de::Error::custom(format!(
            "Size must be string or number, got: {:?}",
            v
        ))),
    }
}

/// Validated order ready for processing
#[derive(Debug, Clone)]
pub struct ValidatedOrder {
    pub id: u64,
    pub coin: String,
    pub is_buy: bool,
    pub price: f64,
    pub size: f64,
    pub status: OrderStatus,
    pub user: String,
    pub timestamp: u64,
    pub is_trigger: bool,
    pub trigger_condition: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OrderStatus {
    Open,
    Filled,
    Canceled,
    Rejected(String),  // Store rejection reason
    Unknown(String),   // Store unknown status
}

impl From<&str> for OrderStatus {
    fn from(s: &str) -> Self {
        match s {
            "open" => OrderStatus::Open,
            "filled" => OrderStatus::Filled,
            "canceled" | "cancelled" => OrderStatus::Canceled,
            s if s.contains("Rejected") => OrderStatus::Rejected(s.to_string()),
            s => OrderStatus::Unknown(s.to_string()),
        }
    }
}

/// Parser with validation and metrics
pub struct OrderParser {
    // Metrics
    total_messages: AtomicU64,
    parse_failures: AtomicU64,
    validation_failures: AtomicU64,
    
    // Configuration
    max_price: f64,
    max_size: f64,
    allowed_coins: Option<Vec<String>>,
}

impl OrderParser {
    pub fn new() -> Self {
        Self {
            total_messages: AtomicU64::new(0),
            parse_failures: AtomicU64::new(0),
            validation_failures: AtomicU64::new(0),
            max_price: 10_000_000.0,  // $10M max
            max_size: 1_000_000.0,     // 1M units max
            allowed_coins: None,
        }
    }
    
    pub fn with_limits(mut self, max_price: f64, max_size: f64) -> Self {
        self.max_price = max_price;
        self.max_size = max_size;
        self
    }
    
    pub fn with_allowed_coins(mut self, coins: Vec<String>) -> Self {
        self.allowed_coins = Some(coins);
        self
    }
    
    /// Parse and validate a JSON line
    pub fn parse_line(&self, line: &str) -> Result<ValidatedOrder> {
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        
        // Parse JSON
        let msg: OrderMessage = match serde_json::from_str(line) {
            Ok(msg) => msg,
            Err(e) => {
                self.parse_failures.fetch_add(1, Ordering::Relaxed);
                
                // Log sample of bad line for debugging
                let sample = &line[..line.len().min(200)];
                error!("JSON parse error: {}, sample: {}...", e, sample);
                
                bail!("Failed to parse JSON: {}", e);
            }
        };
        
        // Validate and convert
        match self.validate_order(msg) {
            Ok(order) => Ok(order),
            Err(e) => {
                self.validation_failures.fetch_add(1, Ordering::Relaxed);
                warn!("Order validation failed: {}", e);
                Err(e)
            }
        }
    }
    
    /// Validate order data
    fn validate_order(&self, msg: OrderMessage) -> Result<ValidatedOrder> {
        let order = &msg.order;
        
        // Validate price
        if order.limit_px <= 0.0 {
            bail!("Invalid price: {} (must be positive)", order.limit_px);
        }
        if order.limit_px > self.max_price {
            bail!("Price too high: {} (max: {})", order.limit_px, self.max_price);
        }
        if order.limit_px.is_nan() || order.limit_px.is_infinite() {
            bail!("Invalid price: {} (NaN or Infinite)", order.limit_px);
        }
        
        // Validate size
        if order.sz <= 0.0 {
            bail!("Invalid size: {} (must be positive)", order.sz);
        }
        if order.sz > self.max_size {
            bail!("Size too large: {} (max: {})", order.sz, self.max_size);
        }
        if order.sz.is_nan() || order.sz.is_infinite() {
            bail!("Invalid size: {} (NaN or Infinite)", order.sz);
        }
        
        // Validate coin
        if order.coin.is_empty() {
            bail!("Empty coin symbol");
        }
        if order.coin.len() > 20 {
            bail!("Coin symbol too long: {}", order.coin);
        }
        if let Some(allowed) = &self.allowed_coins {
            if !allowed.contains(&order.coin) {
                bail!("Unknown coin: {}", order.coin);
            }
        }
        
        // Validate side
        let is_buy = match order.side.as_str() {
            "B" => true,
            "A" => false,
            _ => bail!("Invalid side: {} (expected B or A)", order.side),
        };
        
        // Convert status
        let status = OrderStatus::from(msg.status.as_str());
        
        // Build validated order
        Ok(ValidatedOrder {
            id: order.oid,
            coin: order.coin.clone(),
            is_buy,
            price: order.limit_px,
            size: order.sz,
            status,
            user: msg.user,
            timestamp: order.timestamp,
            is_trigger: order.is_trigger,
            trigger_condition: order.trigger_condition.clone(),
        })
    }
    
    /// Get parser statistics
    pub fn stats(&self) -> ParserStats {
        ParserStats {
            total_messages: self.total_messages.load(Ordering::Relaxed),
            parse_failures: self.parse_failures.load(Ordering::Relaxed),
            validation_failures: self.validation_failures.load(Ordering::Relaxed),
            success_rate: self.calculate_success_rate(),
        }
    }
    
    fn calculate_success_rate(&self) -> f64 {
        let total = self.total_messages.load(Ordering::Relaxed);
        if total == 0 {
            return 100.0;
        }
        
        let failures = self.parse_failures.load(Ordering::Relaxed)
            + self.validation_failures.load(Ordering::Relaxed);
        
        ((total - failures) as f64 / total as f64) * 100.0
    }
}

#[derive(Debug, Clone)]
pub struct ParserStats {
    pub total_messages: u64,
    pub parse_failures: u64,
    pub validation_failures: u64,
    pub success_rate: f64,
}

/// Error recovery buffer for debugging
pub struct ErrorBuffer {
    capacity: usize,
    errors: parking_lot::Mutex<Vec<(String, String, std::time::Instant)>>,
}

impl ErrorBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            errors: parking_lot::Mutex::new(Vec::with_capacity(capacity)),
        }
    }
    
    pub fn add(&self, error: String, sample: String) {
        let mut errors = self.errors.lock();
        
        if errors.len() >= self.capacity {
            errors.remove(0);
        }
        
        errors.push((error, sample, std::time::Instant::now()));
    }
    
    pub fn recent_errors(&self) -> Vec<(String, String, std::time::Duration)> {
        let now = std::time::Instant::now();
        self.errors
            .lock()
            .iter()
            .map(|(err, sample, time)| (err.clone(), sample.clone(), now - *time))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_valid_order() {
        let parser = OrderParser::new();
        
        let json = r#"{
            "order": {
                "oid": 12345,
                "coin": "BTC",
                "side": "B",
                "limitPx": "50000.50",
                "sz": "0.01",
                "isTrigger": false,
                "triggerCondition": "",
                "timestamp": 1234567890
            },
            "status": "open",
            "user": "0x123"
        }"#;
        
        let order = parser.parse_line(json).unwrap();
        assert_eq!(order.id, 12345);
        assert_eq!(order.coin, "BTC");
        assert!(order.is_buy);
        assert_eq!(order.price, 50000.50);
        assert_eq!(order.size, 0.01);
    }
    
    #[test]
    fn test_parse_invalid_price() {
        let parser = OrderParser::new();
        
        let json = r#"{
            "order": {
                "oid": 12345,
                "coin": "BTC",
                "side": "B",
                "limitPx": "-100",
                "sz": "0.01",
                "timestamp": 1234567890
            },
            "status": "open",
            "user": "0x123"
        }"#;
        
        assert!(parser.parse_line(json).is_err());
        assert_eq!(parser.validation_failures.load(Ordering::Relaxed), 1);
    }
    
    #[test]
    fn test_numeric_price() {
        let parser = OrderParser::new();
        
        // Price as number instead of string
        let json = r#"{
            "order": {
                "oid": 12345,
                "coin": "ETH",
                "side": "A",
                "limitPx": 3000.0,
                "sz": 1.5,
                "timestamp": 1234567890
            },
            "status": "filled",
            "user": "0x456"
        }"#;
        
        let order = parser.parse_line(json).unwrap();
        assert_eq!(order.price, 3000.0);
        assert_eq!(order.size, 1.5);
    }
}