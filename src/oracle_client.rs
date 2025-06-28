use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration, Instant};

#[derive(Debug, Clone)]
pub struct OraclePrice {
    pub symbol: String,
    pub price: f64,
    pub timestamp: Instant,
}

// allMids response is a HashMap<String, String> where keys are asset names
type AllMidsResponse = HashMap<String, String>;

pub struct OracleClient {
    client: Client,
    cache: Arc<RwLock<HashMap<String, OraclePrice>>>,
    api_url: String,
}

impl OracleClient {
    pub fn new() -> Self {
        // Use HTTP/2 for lower latency
        let client = Client::builder()
            .http2_prior_knowledge()
            .pool_idle_timeout(Duration::from_secs(30))
            .pool_max_idle_per_host(10)
            .timeout(Duration::from_millis(500))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
            api_url: "https://api.hyperliquid.xyz/info".to_string(),
        }
    }

    pub async fn get_oracle_price(&self, symbol: &str) -> Option<f64> {
        // Check cache first (valid for 2.5 seconds since oracle updates every 3s)
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(symbol) {
                if cached.timestamp.elapsed() < Duration::from_millis(2500) {
                    return Some(cached.price);
                }
            }
        }

        // Fetch fresh prices
        if let Ok(prices) = self.fetch_all_oracle_prices().await {
            let mut cache = self.cache.write().await;
            let now = Instant::now();
            
            for (sym, price) in prices {
                cache.insert(sym.clone(), OraclePrice {
                    symbol: sym.clone(),
                    price,
                    timestamp: now,
                });
                
                if sym == symbol {
                    return Some(price);
                }
            }
        }

        None
    }

    pub async fn fetch_all_oracle_prices(&self) -> Result<HashMap<String, f64>, reqwest::Error> {
        let start = Instant::now();
        
        let response = self.client
            .post(&self.api_url)
            .json(&serde_json::json!({"type": "allMids"}))
            .send()
            .await?;

        let data: AllMidsResponse = response.json().await?;
        
        let mut prices = HashMap::new();
        for (symbol, price_str) in data {
            // Skip keys that start with "@" (these are numeric indices)
            if symbol.starts_with('@') {
                continue;
            }
            
            if let Ok(price) = price_str.parse::<f64>() {
                prices.insert(symbol, price);
            }
        }

        let latency = start.elapsed();
        log::debug!("Oracle price fetch latency: {:?}", latency);

        Ok(prices)
    }

    pub async fn start_oracle_feed(&self, update_interval: Duration) {
        let cache = self.cache.clone();
        let client = self.client.clone();
        let api_url = self.api_url.clone();

        tokio::spawn(async move {
            let mut ticker = interval(update_interval);
            
            loop {
                ticker.tick().await;
                
                let start = Instant::now();
                
                match client
                    .post(&api_url)
                    .json(&serde_json::json!({"type": "allMids"}))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if let Ok(data) = response.json::<AllMidsResponse>().await {
                            let mut new_cache = HashMap::new();
                            let now = Instant::now();
                            
                            for (symbol, price_str) in data {
                                // Skip numeric indices
                                if symbol.starts_with('@') {
                                    continue;
                                }
                                
                                if let Ok(price) = price_str.parse::<f64>() {
                                    new_cache.insert(symbol.clone(), OraclePrice {
                                        symbol,
                                        price,
                                        timestamp: now,
                                    });
                                }
                            }
                            
                            let cache_size = new_cache.len();
                            let mut cache_write = cache.write().await;
                            *cache_write = new_cache;
                            
                            let latency = start.elapsed();
                            log::info!("Oracle prices updated. {} assets, latency: {:?}", cache_size, latency);
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to fetch oracle prices: {}", e);
                    }
                }
            }
        });
    }

    pub async fn get_all_cached_prices(&self) -> HashMap<String, f64> {
        let cache = self.cache.read().await;
        cache.iter()
            .map(|(k, v)| (k.clone(), v.price))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_oracle_client() {
        let client = OracleClient::new();
        
        // Start the background feed
        client.start_oracle_feed(Duration::from_secs(3)).await;
        
        // Wait a bit for initial fetch
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Test getting BTC price
        if let Some(btc_price) = client.get_oracle_price("BTC").await {
            println!("BTC Oracle Price: ${:.2}", btc_price);
            assert!(btc_price > 0.0);
        }
        
        // Test cache hit (should be very fast)
        let start = Instant::now();
        client.get_oracle_price("BTC").await;
        let cache_latency = start.elapsed();
        println!("Cache hit latency: {:?}", cache_latency);
        assert!(cache_latency < Duration::from_millis(1));
    }
}