use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use tracing::{info, error, warn};

/// Oracle price data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OraclePrice {
    pub symbol: String,
    pub price: f64,
    pub timestamp: u64,
}

/// Oracle price feed manager
pub struct OraclePriceFeed {
    client: Client,
    prices: Arc<RwLock<HashMap<String, OraclePrice>>>,
    api_url: String,
    polling_interval: Duration,
}

#[derive(Debug, Deserialize)]
struct MetaResponse {
    universe: Vec<AssetInfo>,
}

#[derive(Debug, Deserialize)]
struct AssetInfo {
    name: String,
    #[serde(rename = "markPx")]
    mark_px: String,
}

impl OraclePriceFeed {
    pub fn new() -> Self {
        // Create HTTP client with connection pooling
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_secs(5))
            .tcp_keepalive(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            prices: Arc::new(RwLock::new(HashMap::new())),
            api_url: "https://api.hyperliquid.xyz/info".to_string(),
            polling_interval: Duration::from_secs(3), // 3 seconds as per Hyperliquid docs
        }
    }

    /// Start the oracle price feed
    pub async fn start(&self) {
        let prices = self.prices.clone();
        let client = self.client.clone();
        let api_url = self.api_url.clone();
        let polling_interval = self.polling_interval;

        tokio::spawn(async move {
            let mut interval_timer = interval(polling_interval);
            
            loop {
                interval_timer.tick().await;
                
                match Self::fetch_oracle_prices(&client, &api_url).await {
                    Ok(new_prices) => {
                        let mut price_map = prices.write().await;
                        *price_map = new_prices;
                        info!("Updated {} oracle prices", price_map.len());
                    }
                    Err(e) => {
                        error!("Failed to fetch oracle prices: {}", e);
                    }
                }
            }
        });

        info!("Oracle price feed started");
    }

    /// Fetch oracle prices from Hyperliquid API
    async fn fetch_oracle_prices(
        client: &Client,
        api_url: &str,
    ) -> Result<HashMap<String, OraclePrice>, Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        
        let response = client
            .post(api_url)
            .json(&serde_json::json!({ "type": "meta" }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("API error: {}", response.status()).into());
        }

        let data: MetaResponse = response.json().await?;
        let latency_ms = start.elapsed().as_millis();
        
        let mut oracle_prices = HashMap::new();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for asset in data.universe {
            if let Ok(price) = asset.mark_px.parse::<f64>() {
                oracle_prices.insert(
                    asset.name.clone(),
                    OraclePrice {
                        symbol: asset.name,
                        price,
                        timestamp,
                    },
                );
            }
        }

        info!("Fetched {} oracle prices in {}ms", oracle_prices.len(), latency_ms);
        Ok(oracle_prices)
    }

    /// Get oracle price for a specific symbol
    pub async fn get_price(&self, symbol: &str) -> Option<f64> {
        let prices = self.prices.read().await;
        prices.get(symbol).map(|p| p.price)
    }

    /// Get all oracle prices
    pub async fn get_all_prices(&self) -> HashMap<String, f64> {
        let prices = self.prices.read().await;
        prices
            .iter()
            .map(|(symbol, price)| (symbol.clone(), price.price))
            .collect()
    }

    /// Update orderbook with oracle price
    pub async fn update_orderbook_oracle_price(
        &self,
        orderbook: &crate::fast_orderbook::FastOrderbook,
        symbol: &str,
    ) {
        if let Some(price) = self.get_price(symbol).await {
            orderbook.update_oracle_price(price);
            info!("Updated {} orderbook with oracle price: ${:.2}", symbol, price);
        }
    }
}

/// Integration with the main orderbook service
pub struct OraclePriceIntegration {
    feed: Arc<OraclePriceFeed>,
    orderbooks: Arc<HashMap<String, Arc<crate::fast_orderbook::FastOrderbook>>>,
}

impl OraclePriceIntegration {
    pub fn new(
        orderbooks: Arc<HashMap<String, Arc<crate::fast_orderbook::FastOrderbook>>>,
    ) -> Self {
        Self {
            feed: Arc::new(OraclePriceFeed::new()),
            orderbooks,
        }
    }

    /// Start oracle price integration
    pub async fn start(&self) {
        // Start the price feed
        self.feed.start().await;

        // Start update loop
        let feed = self.feed.clone();
        let orderbooks = self.orderbooks.clone();

        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(3));
            
            loop {
                interval_timer.tick().await;
                
                // Update all orderbooks with latest oracle prices
                let prices = feed.get_all_prices().await;
                
                for (symbol, orderbook) in orderbooks.iter() {
                    if let Some(price) = prices.get(symbol) {
                        orderbook.update_oracle_price(*price);
                    }
                }
            }
        });

        info!("Oracle price integration started");
    }
}

/// Low-latency oracle price client using persistent connections
pub struct LowLatencyOracleClient {
    client: Client,
    api_url: String,
}

impl LowLatencyOracleClient {
    pub fn new() -> Self {
        // Optimized client for lowest latency
        let client = Client::builder()
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(20)
            .timeout(Duration::from_millis(2000))
            .tcp_keepalive(Duration::from_secs(30))
            .tcp_nodelay(true) // Disable Nagle's algorithm
            .http2_prior_knowledge() // Use HTTP/2 if available
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            api_url: "https://api.hyperliquid.xyz/info".to_string(),
        }
    }

    /// Fetch oracle price with minimal latency
    pub async fn get_oracle_price(&self, symbol: &str) -> Result<f64, Box<dyn std::error::Error>> {
        let start = std::time::Instant::now();
        
        let response = self
            .client
            .post(&self.api_url)
            .json(&serde_json::json!({ "type": "meta" }))
            .send()
            .await?;

        let data: MetaResponse = response.json().await?;
        let latency_us = start.elapsed().as_micros();

        for asset in data.universe {
            if asset.name == symbol {
                let price = asset.mark_px.parse::<f64>()?;
                info!("Fetched {} oracle price in {}μs", symbol, latency_us);
                return Ok(price);
            }
        }

        Err(format!("Symbol {} not found", symbol).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oracle_price_feed() {
        let feed = OraclePriceFeed::new();
        
        // Test fetching prices
        let prices = OraclePriceFeed::fetch_oracle_prices(
            &feed.client,
            &feed.api_url,
        ).await;
        
        assert!(prices.is_ok());
        let prices = prices.unwrap();
        assert!(!prices.is_empty());
        
        // Check BTC price exists
        assert!(prices.contains_key("BTC"));
        let btc_price = &prices["BTC"];
        assert!(btc_price.price > 0.0);
    }

    #[tokio::test]
    async fn test_low_latency_client() {
        let client = LowLatencyOracleClient::new();
        
        // Test fetching BTC price
        let result = client.get_oracle_price("BTC").await;
        assert!(result.is_ok());
        
        let price = result.unwrap();
        assert!(price > 0.0);
        println!("BTC oracle price: ${:.2}", price);
    }
}