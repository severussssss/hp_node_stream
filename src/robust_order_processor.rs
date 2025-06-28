use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::fast_orderbook::{FastOrderbook, OrderbookDelta, Order};
use crate::market_processor::MarketUpdate;
use crate::markets;
use crate::dynamic_markets::DynamicMarketRegistry;
use crate::order_parser::{OrderParser, ValidatedOrder, OrderStatus};
use crate::stop_orders::{StopOrderManager, StopOrder};
use crate::per_market_circuit_breaker::{PerMarketCircuitBreaker, CircuitBreakerConfig};

/// Configuration for robust order processing
pub struct ProcessorConfig {
    pub max_price: f64,
    pub max_size: f64,
    pub error_threshold: u32,
    pub error_window: Duration,
    pub log_sample_rate: u32,  // Log 1 in N errors
}

impl Default for ProcessorConfig {
    fn default() -> Self {
        Self {
            max_price: 10_000_000.0,  // $10M
            max_size: 1_000_000.0,     // 1M units
            error_threshold: 100,       // Trip circuit after 100 errors
            error_window: Duration::from_secs(60),  // Per minute
            log_sample_rate: 10,        // Log every 10th error
        }
    }
}

/// Robust order processor with error recovery
pub struct RobustOrderProcessor {
    parser: Arc<OrderParser>,
    config: ProcessorConfig,
    error_buffer: Arc<crate::order_parser::ErrorBuffer>,
    circuit_breaker: Arc<PerMarketCircuitBreaker>,
    market_registry: Arc<DynamicMarketRegistry>,
}

impl RobustOrderProcessor {
    pub fn new(config: ProcessorConfig, market_registry: Arc<DynamicMarketRegistry>) -> Self {
        // No need for static allowed_coins list anymore
        let parser = OrderParser::new()
            .with_limits(config.max_price, config.max_size)
            .with_allowed_coins(vec![]); // Will use dynamic registry instead
        
        let cb_config = CircuitBreakerConfig {
            failure_threshold: 10,  // Per-market threshold
            success_threshold: 3,
            timeout: Duration::from_secs(30),
            error_window: config.error_window,
        };
        
        Self {
            parser: Arc::new(parser),
            config,
            error_buffer: Arc::new(crate::order_parser::ErrorBuffer::new(100)),
            circuit_breaker: Arc::new(PerMarketCircuitBreaker::new(cb_config)),
            market_registry,
        }
    }
    
    pub async fn start(
        self: Arc<Self>,
        data_path: String,
        orderbooks: Arc<std::collections::HashMap<u32, Arc<FastOrderbook>>>,
        update_tx: broadcast::Sender<MarketUpdate>,
        stop_order_manager: Arc<StopOrderManager>,
    ) -> Result<()> {
        info!("Starting robust order processor for: {}", data_path);
        
        // Start monitoring task
        let monitor_self = self.clone();
        tokio::spawn(async move {
            monitor_self.monitor_stats().await;
        });
        
        // Main processing loop
        self.process_orders(data_path, orderbooks, update_tx, stop_order_manager).await
    }
    
    async fn process_orders(
        &self,
        data_path: String,
        orderbooks: Arc<std::collections::HashMap<u32, Arc<FastOrderbook>>>,
        update_tx: broadcast::Sender<MarketUpdate>,
        stop_order_manager: Arc<StopOrderManager>,
    ) -> Result<()> {
        // Start tailing the file
        let mut cmd = Command::new("docker")
            .args(&["exec", "hyperliquid-node-1", "tail", "-n", "0", "-f", &data_path])
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        
        let stdout = cmd.stdout.take().expect("Failed to get stdout");
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        
        let mut error_count = 0u32;
        let mut window_start = Instant::now();
        let mut order_count = 0u64;
        let start_time = Instant::now();
        
        while let Ok(Some(line)) = lines.next_line().await {
            // Reset error window
            if window_start.elapsed() > self.config.error_window {
                error_count = 0;
                window_start = Instant::now();
            }
            
            // Process line with per-market circuit breaker
            match self.process_single_order_with_circuit_breaker(&line, &orderbooks, &update_tx, &stop_order_manager).await {
                Ok(processed) => {
                    if processed {
                        order_count += 1;
                        
                        // Log progress
                        if order_count % 1000 == 0 {
                            let elapsed = start_time.elapsed().as_secs_f64();
                            let rate = order_count as f64 / elapsed;
                            let stats = self.parser.stats();
                            
                            info!(
                                "Processed {} orders, {:.0} orders/sec, success rate: {:.1}%",
                                order_count, rate, stats.success_rate
                            );
                        }
                    }
                }
                Err(e) => {
                    error_count += 1;
                    self.error_buffer.add(e.to_string(), line.clone());
                    
                    // Sample error logging
                    if error_count % self.config.log_sample_rate == 1 {
                        let recent_errors = self.error_buffer.recent_errors();
                        error!(
                            "Order processing error: {}, recent errors: {} in last minute",
                            e,
                            recent_errors.len()
                        );
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_single_order_with_circuit_breaker(
        &self,
        line: &str,
        orderbooks: &Arc<std::collections::HashMap<u32, Arc<FastOrderbook>>>,
        update_tx: &broadcast::Sender<MarketUpdate>,
        stop_order_manager: &Arc<StopOrderManager>,
    ) -> Result<bool> {
        // First parse to check what we're dealing with
        let order = match self.parser.parse_line(line) {
            Ok(order) => order,
            Err(e) => {
                // Validation errors (size, price) go to validation circuit
                self.circuit_breaker.record_validation_failure(e.to_string());
                return Err(e);
            }
        };
        
        // Try to get market ID
        match self.market_registry.get_market_id(&order.coin).await {
            Some(market_id) => {
                // Check if this market's circuit is open
                if self.circuit_breaker.is_market_open(market_id) {
                    // Check if we should reset
                    if self.circuit_breaker.should_attempt_market_reset(market_id) {
                        self.circuit_breaker.attempt_market_reset(market_id);
                        info!("Attempting to reset circuit breaker for market {}", market_id);
                    } else {
                        // Skip this order, circuit is open
                        return Ok(false);
                    }
                }
                
                // Process the order
                match self.process_market_order(order, market_id, orderbooks, update_tx, stop_order_manager).await {
                    Ok(processed) => {
                        if processed {
                            self.circuit_breaker.record_market_success(market_id);
                        }
                        Ok(processed)
                    }
                    Err(e) => {
                        self.circuit_breaker.record_market_failure(market_id, e.to_string());
                        Err(e)
                    }
                }
            }
            None => {
                // Unknown market - check validation circuit
                if self.circuit_breaker.is_validation_circuit_open() {
                    return Ok(false); // Skip unknown markets when validation circuit is open
                }
                
                let err = anyhow::anyhow!("Unknown market: {}", order.coin);
                self.circuit_breaker.record_validation_failure(err.to_string());
                Err(err)
            }
        }
    }
    
    async fn process_market_order(
        &self,
        order: ValidatedOrder,
        market_id: u32,
        orderbooks: &Arc<std::collections::HashMap<u32, Arc<FastOrderbook>>>,
        update_tx: &broadcast::Sender<MarketUpdate>,
        stop_order_manager: &Arc<StopOrderManager>,
    ) -> Result<bool> {
        // Get orderbook
        let orderbook = orderbooks.get(&market_id)
            .ok_or_else(|| anyhow::anyhow!("No orderbook for market {}", market_id))?;
        
        // Process based on order type
        let delta = self.process_validated_order(order, orderbook, stop_order_manager, market_id)?;
        
        if let Some(delta) = delta {
            // Send update
            let update = MarketUpdate {
                market_id,
                sequence: orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                timestamp_ns: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64,
                deltas: vec![delta],
            };
            
            let _ = update_tx.send(update);
            Ok(true)
        } else {
            Ok(false)
        }
    }
    
    fn process_validated_order(
        &self,
        order: ValidatedOrder,
        orderbook: &Arc<FastOrderbook>,
        stop_order_manager: &Arc<StopOrderManager>,
        market_id: u32,
    ) -> Result<Option<OrderbookDelta>> {
        // Skip rejected orders
        if matches!(order.status, OrderStatus::Rejected(_)) {
            return Ok(None);
        }
        
        // Handle trigger/stop orders
        if order.is_trigger {
            if matches!(order.status, OrderStatus::Open) {
                let stop_order = StopOrder {
                    id: order.id,
                    user: order.user,
                    coin: order.coin,
                    side: if order.is_buy { "B" } else { "A" }.to_string(),
                    price: order.price,
                    size: order.size,
                    trigger_condition: order.trigger_condition,
                    timestamp: order.timestamp,
                };
                stop_order_manager.add_stop_order(market_id, stop_order);
            }
            return Ok(None);
        }
        
        // Process regular orders
        match order.status {
            OrderStatus::Open => {
                let book_order = Order {
                    id: order.id,
                    price: order.price,
                    size: order.size,
                    timestamp: order.timestamp,
                };
                
                let delta = orderbook.add_order(book_order, order.is_buy);
                Ok(Some(delta))
            }
            OrderStatus::Filled | OrderStatus::Canceled => {
                Ok(orderbook.remove_order(order.id, order.price, order.is_buy))
            }
            _ => Ok(None),
        }
    }
    
    async fn monitor_stats(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            let stats = self.parser.stats();
            let cb_stats = self.circuit_breaker.get_stats();
            
            info!(
                "Parser stats - Total: {}, Parse errors: {}, Validation errors: {}, Success rate: {:.1}%",
                stats.total_messages,
                stats.parse_failures,
                stats.validation_failures,
                stats.success_rate
            );
            
            info!(
                "Circuit breaker stats - Open markets: {} ({}), Validation circuit: {}, Total markets: {}",
                cb_stats.open_markets.len(),
                cb_stats.open_markets.iter()
                    .map(|(id, reason)| format!("{}: {}", id, reason))
                    .collect::<Vec<_>>()
                    .join(", "),
                cb_stats.validation_circuit_state,
                cb_stats.total_markets
            );
            
            // Alert if success rate is low
            if stats.success_rate < 95.0 && stats.total_messages > 1000 {
                warn!(
                    "Low parser success rate: {:.1}% (threshold: 95%)",
                    stats.success_rate
                );
            }
        }
    }
}

