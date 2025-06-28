use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use parking_lot::RwLock;
use tracing::{info, warn, error};

/// Circuit breaker configuration
#[derive(Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub error_window: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 10,      // Trip after 10 consecutive failures
            success_threshold: 3,       // Need 3 successes to fully close
            timeout: Duration::from_secs(30),
            error_window: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
enum CircuitState {
    Closed { 
        consecutive_failures: u32,
    },
    Open { 
        since: Instant,
        failure_reason: String,
    },
    HalfOpen {
        consecutive_successes: u32,
    },
}

/// Per-market circuit breaker
struct MarketCircuitBreaker {
    state: CircuitState,
    last_failure_time: Option<Instant>,
    total_failures: u32,
    total_successes: u32,
}

impl MarketCircuitBreaker {
    fn new() -> Self {
        Self {
            state: CircuitState::Closed { consecutive_failures: 0 },
            last_failure_time: None,
            total_failures: 0,
            total_successes: 0,
        }
    }
}

/// Manages circuit breakers for all markets
pub struct PerMarketCircuitBreaker {
    breakers: Arc<RwLock<HashMap<u32, MarketCircuitBreaker>>>,
    config: CircuitBreakerConfig,
    global_validation_breaker: Arc<RwLock<MarketCircuitBreaker>>, // For validation errors
}

impl PerMarketCircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            breakers: Arc::new(RwLock::new(HashMap::new())),
            config,
            global_validation_breaker: Arc::new(RwLock::new(MarketCircuitBreaker::new())),
        }
    }

    /// Check if a specific market's circuit is open
    pub fn is_market_open(&self, market_id: u32) -> bool {
        let breakers = self.breakers.read();
        if let Some(breaker) = breakers.get(&market_id) {
            matches!(breaker.state, CircuitState::Open { .. })
        } else {
            false
        }
    }

    /// Check if validation circuit is open (for unknown markets, size violations)
    pub fn is_validation_circuit_open(&self) -> bool {
        let breaker = self.global_validation_breaker.read();
        matches!(breaker.state, CircuitState::Open { .. })
    }

    /// Record a successful order for a market
    pub fn record_market_success(&self, market_id: u32) {
        let mut breakers = self.breakers.write();
        let breaker = breakers.entry(market_id).or_insert_with(MarketCircuitBreaker::new);
        
        breaker.total_successes += 1;
        
        match &mut breaker.state {
            CircuitState::Closed { consecutive_failures } => {
                *consecutive_failures = 0;
            }
            CircuitState::HalfOpen { consecutive_successes } => {
                *consecutive_successes += 1;
                if *consecutive_successes >= self.config.success_threshold {
                    breaker.state = CircuitState::Closed { consecutive_failures: 0 };
                    info!("Circuit breaker closed for market {}", market_id);
                }
            }
            _ => {}
        }
    }

    /// Record a failure for a market
    pub fn record_market_failure(&self, market_id: u32, reason: String) {
        let mut breakers = self.breakers.write();
        let breaker = breakers.entry(market_id).or_insert_with(MarketCircuitBreaker::new);
        
        breaker.total_failures += 1;
        breaker.last_failure_time = Some(Instant::now());
        
        match &mut breaker.state {
            CircuitState::Closed { consecutive_failures } => {
                *consecutive_failures += 1;
                if *consecutive_failures >= self.config.failure_threshold {
                    breaker.state = CircuitState::Open { 
                        since: Instant::now(),
                        failure_reason: reason.clone(),
                    };
                    error!("Circuit breaker tripped for market {} - {}", market_id, reason);
                }
            }
            CircuitState::HalfOpen { .. } => {
                breaker.state = CircuitState::Open { 
                    since: Instant::now(),
                    failure_reason: reason.clone(),
                };
                warn!("Circuit breaker re-opened for market {} - {}", market_id, reason);
            }
            _ => {}
        }
    }

    /// Record a validation failure (unknown market, size violation)
    pub fn record_validation_failure(&self, reason: String) {
        let mut breaker = self.global_validation_breaker.write();
        
        breaker.total_failures += 1;
        breaker.last_failure_time = Some(Instant::now());
        
        match &mut breaker.state {
            CircuitState::Closed { consecutive_failures } => {
                *consecutive_failures += 1;
                if *consecutive_failures >= self.config.failure_threshold {
                    breaker.state = CircuitState::Open { 
                        since: Instant::now(),
                        failure_reason: reason.clone(),
                    };
                    error!("Validation circuit breaker tripped - {}", reason);
                }
            }
            CircuitState::HalfOpen { .. } => {
                breaker.state = CircuitState::Open { 
                    since: Instant::now(),
                    failure_reason: reason.clone(),
                };
                warn!("Validation circuit breaker re-opened - {}", reason);
            }
            _ => {}
        }
    }

    /// Check if a market should attempt reset
    pub fn should_attempt_market_reset(&self, market_id: u32) -> bool {
        let breakers = self.breakers.read();
        if let Some(breaker) = breakers.get(&market_id) {
            if let CircuitState::Open { since, .. } = breaker.state {
                return since.elapsed() > self.config.timeout;
            }
        }
        false
    }

    /// Attempt to reset a market's circuit
    pub fn attempt_market_reset(&self, market_id: u32) {
        let mut breakers = self.breakers.write();
        if let Some(breaker) = breakers.get_mut(&market_id) {
            if let CircuitState::Open { since, .. } = breaker.state {
                if since.elapsed() > self.config.timeout {
                    breaker.state = CircuitState::HalfOpen { consecutive_successes: 0 };
                    info!("Circuit breaker moved to half-open for market {}", market_id);
                }
            }
        }
    }

    /// Get circuit statistics
    pub fn get_stats(&self) -> CircuitBreakerStats {
        let breakers = self.breakers.read();
        let validation_breaker = self.global_validation_breaker.read();
        
        let mut open_markets = Vec::new();
        let mut half_open_markets = Vec::new();
        let mut closed_markets = 0;
        
        for (market_id, breaker) in breakers.iter() {
            match &breaker.state {
                CircuitState::Open { failure_reason, .. } => {
                    open_markets.push((*market_id, failure_reason.clone()));
                }
                CircuitState::HalfOpen { .. } => {
                    half_open_markets.push(*market_id);
                }
                CircuitState::Closed { .. } => {
                    closed_markets += 1;
                }
            }
        }
        
        let validation_state = match &validation_breaker.state {
            CircuitState::Open { .. } => "OPEN",
            CircuitState::HalfOpen { .. } => "HALF-OPEN",
            CircuitState::Closed { .. } => "CLOSED",
        };
        
        CircuitBreakerStats {
            total_markets: breakers.len(),
            open_markets,
            half_open_markets,
            closed_markets,
            validation_circuit_state: validation_state.to_string(),
            validation_failures: validation_breaker.total_failures,
        }
    }
}

#[derive(Debug)]
pub struct CircuitBreakerStats {
    pub total_markets: usize,
    pub open_markets: Vec<(u32, String)>,
    pub half_open_markets: Vec<u32>,
    pub closed_markets: usize,
    pub validation_circuit_state: String,
    pub validation_failures: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_per_market_isolation() {
        let cb = PerMarketCircuitBreaker::new(CircuitBreakerConfig::default());
        
        // BTC market failures shouldn't affect ETH
        for _ in 0..10 {
            cb.record_market_failure(0, "test failure".to_string()); // BTC
        }
        
        assert!(cb.is_market_open(0));  // BTC circuit should be open
        assert!(!cb.is_market_open(1)); // ETH circuit should be closed
        
        // ETH should still work
        cb.record_market_success(1);
        assert!(!cb.is_market_open(1));
    }
    
    #[test]
    fn test_validation_circuit_separate() {
        let cb = PerMarketCircuitBreaker::new(CircuitBreakerConfig::default());
        
        // Validation failures (unknown coins) shouldn't affect known markets
        for _ in 0..10 {
            cb.record_validation_failure("Unknown coin".to_string());
        }
        
        assert!(cb.is_validation_circuit_open());
        assert!(!cb.is_market_open(0)); // BTC should still work
        assert!(!cb.is_market_open(1)); // ETH should still work
    }
}