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
use crate::order_parser::{OrderParser, ValidatedOrder, OrderStatus};
use crate::stop_orders::{StopOrderManager, StopOrder};

/// Configuration for robust order processing
pub struct ProcessorConfig {
    pub max_price: f64,
    pub max_size: f64,
    pub error_threshold: u32,
    pub error_window: Duration,
    pub log_sample_rate: u64,  // Log 1 in N errors
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
    circuit_breaker: Arc<CircuitBreaker>,
}

impl RobustOrderProcessor {
    pub fn new(config: ProcessorConfig, allowed_coins: Vec<String>) -> Self {
        let parser = OrderParser::new()
            .with_limits(config.max_price, config.max_size)
            .with_allowed_coins(allowed_coins);
        
        Self {
            parser: Arc::new(parser),
            config,
            error_buffer: Arc::new(crate::order_parser::ErrorBuffer::new(100)),
            circuit_breaker: Arc::new(CircuitBreaker::new()),
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
            .args(&["exec", "hyperliquid-node-1", "tail", "-f", &data_path])
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
            
            // Check circuit breaker
            if self.circuit_breaker.is_open() {
                if self.circuit_breaker.should_attempt_reset() {
                    info!("Attempting to reset circuit breaker");
                    self.circuit_breaker.attempt_reset();
                } else {
                    continue;  // Skip processing while circuit is open
                }
            }
            
            // Process line
            match self.process_single_order(&line, &orderbooks, &update_tx, &stop_order_manager).await {
                Ok(processed) => {
                    if processed {
                        order_count += 1;
                        self.circuit_breaker.record_success();
                        
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
                    self.circuit_breaker.record_failure();
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
                    
                    // Trip circuit if threshold exceeded
                    if error_count >= self.config.error_threshold {
                        error!(
                            "Error threshold exceeded ({}/{}), tripping circuit breaker",
                            error_count, self.config.error_threshold
                        );
                        self.circuit_breaker.trip();
                    }
                }
            }
        }
        
        Ok(())
    }
    
    async fn process_single_order(
        &self,
        line: &str,
        orderbooks: &Arc<std::collections::HashMap<u32, Arc<FastOrderbook>>>,
        update_tx: &broadcast::Sender<MarketUpdate>,
        stop_order_manager: &Arc<StopOrderManager>,
    ) -> Result<bool> {
        // Parse and validate
        let order = self.parser.parse_line(line)?;
        
        // Get market ID
        let market_id = markets::get_market_id(&order.coin)
            .ok_or_else(|| anyhow::anyhow!("Unknown market: {}", order.coin))?;
        
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
                
                if order.is_buy {
                    orderbook.add_bid(book_order);
                    Ok(Some(OrderbookDelta::AddBid {
                        price: order.price,
                        size: order.size,
                        order_id: order.id,
                    }))
                } else {
                    orderbook.add_ask(book_order);
                    Ok(Some(OrderbookDelta::AddAsk {
                        price: order.price,
                        size: order.size,
                        order_id: order.id,
                    }))
                }
            }
            OrderStatus::Filled | OrderStatus::Canceled => {
                if order.is_buy {
                    orderbook.remove_bid(order.id);
                    Ok(Some(OrderbookDelta::RemoveBid {
                        price: order.price,
                        order_id: order.id,
                    }))
                } else {
                    orderbook.remove_ask(order.id);
                    Ok(Some(OrderbookDelta::RemoveAsk {
                        price: order.price,
                        order_id: order.id,
                    }))
                }
            }
            _ => Ok(None),
        }
    }
    
    async fn monitor_stats(&self) {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        
        loop {
            interval.tick().await;
            
            let stats = self.parser.stats();
            let circuit_state = if self.circuit_breaker.is_open() {
                "OPEN"
            } else {
                "CLOSED"
            };
            
            info!(
                "Parser stats - Total: {}, Parse errors: {}, Validation errors: {}, Success rate: {:.1}%, Circuit: {}",
                stats.total_messages,
                stats.parse_failures,
                stats.validation_failures,
                stats.success_rate,
                circuit_state
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

/// Simple circuit breaker implementation
pub struct CircuitBreaker {
    state: parking_lot::RwLock<CircuitState>,
    failure_threshold: u32,
    success_threshold: u32,
    timeout: Duration,
}

#[derive(Debug, Clone)]
enum CircuitState {
    Closed,
    Open { since: Instant },
    HalfOpen,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self {
            state: parking_lot::RwLock::new(CircuitState::Closed),
            failure_threshold: 5,
            success_threshold: 3,
            timeout: Duration::from_secs(30),
        }
    }
    
    pub fn is_open(&self) -> bool {
        matches!(*self.state.read(), CircuitState::Open { .. })
    }
    
    pub fn should_attempt_reset(&self) -> bool {
        match *self.state.read() {
            CircuitState::Open { since } => since.elapsed() > self.timeout,
            _ => false,
        }
    }
    
    pub fn attempt_reset(&self) {
        let mut state = self.state.write();
        if let CircuitState::Open { since } = *state {
            if since.elapsed() > self.timeout {
                *state = CircuitState::HalfOpen;
            }
        }
    }
    
    pub fn record_success(&self) {
        let mut state = self.state.write();
        match *state {
            CircuitState::HalfOpen => {
                // Could track consecutive successes before fully closing
                *state = CircuitState::Closed;
                info!("Circuit breaker closed");
            }
            _ => {}
        }
    }
    
    pub fn record_failure(&self) {
        let mut state = self.state.write();
        match *state {
            CircuitState::HalfOpen => {
                *state = CircuitState::Open { since: Instant::now() };
                warn!("Circuit breaker re-opened");
            }
            _ => {}
        }
    }
    
    pub fn trip(&self) {
        let mut state = self.state.write();
        *state = CircuitState::Open { since: Instant::now() };
        error!("Circuit breaker tripped");
    }
}