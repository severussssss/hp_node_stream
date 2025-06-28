use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};
use crate::symbology::{TradableProduct, MarketInfo, ProductInfo, ExecutionInfo, SymbologyService};

#[derive(Debug, Deserialize)]
struct HyperliquidMeta {
    universe: Vec<AssetInfo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AssetInfo {
    pub name: String,
    #[serde(rename = "isDelisted")]
    pub is_delisted: Option<bool>,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: Option<u32>,
    #[serde(rename = "szDecimals")]
    pub sz_decimals: Option<u32>,
}


pub struct DynamicMarketRegistry {
    markets: Arc<RwLock<HashMap<u32, String>>>,
    coin_to_id: Arc<RwLock<HashMap<String, u32>>>,
    market_info: Arc<RwLock<HashMap<TradableProduct, MarketInfo>>>,
    symbol_to_id: Arc<RwLock<HashMap<TradableProduct, u32>>>,
    last_update: Arc<RwLock<std::time::Instant>>,
}

impl DynamicMarketRegistry {
    pub fn new() -> Self {
        Self {
            markets: Arc::new(RwLock::new(HashMap::new())),
            coin_to_id: Arc::new(RwLock::new(HashMap::new())),
            market_info: Arc::new(RwLock::new(HashMap::new())),
            symbol_to_id: Arc::new(RwLock::new(HashMap::new())),
            last_update: Arc::new(RwLock::new(std::time::Instant::now())),
        }
    }

    pub async fn refresh_markets(&self) -> Result<()> {
        info!("Fetching latest market list from Hyperliquid");
        
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.hyperliquid.xyz/info")
            .json(&serde_json::json!({"type": "meta"}))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await?;
        
        let meta: HyperliquidMeta = response.json().await?;
        
        let mut new_markets = HashMap::new();
        let mut new_coin_to_id = HashMap::new();
        let mut new_market_info = HashMap::new();
        let mut new_symbol_to_id = HashMap::new();
        let mut active_count = 0;
        
        for (id, asset) in meta.universe.iter().enumerate() {
            if !asset.is_delisted.unwrap_or(false) {
                let id = id as u32;
                new_markets.insert(id, asset.name.clone());
                new_coin_to_id.insert(asset.name.clone(), id);
                
                // Create MarketInfo with symbology
                let market_info = MarketInfo::from_hyperliquid(
                    id,
                    asset.name.clone(),
                    asset.max_leverage.unwrap_or(1),
                    asset.sz_decimals.unwrap_or(0),
                    false,
                );
                
                let symbol = market_info.symbol.clone();
                new_market_info.insert(symbol.clone(), market_info);
                new_symbol_to_id.insert(symbol, id);
                
                active_count += 1;
            }
        }
        
        info!(
            "Found {} active markets out of {} total", 
            active_count, 
            meta.universe.len()
        );
        
        // Update atomically
        *self.markets.write().await = new_markets;
        *self.coin_to_id.write().await = new_coin_to_id;
        *self.market_info.write().await = new_market_info;
        *self.symbol_to_id.write().await = new_symbol_to_id;
        *self.last_update.write().await = std::time::Instant::now();
        
        Ok(())
    }
    
    pub async fn get_market_id(&self, coin: &str) -> Option<u32> {
        // First try direct coin lookup (backward compatibility)
        if let Some(id) = self.coin_to_id.read().await.get(coin).copied() {
            return Some(id);
        }
        
        // Try as TradableProduct symbol
        if let Ok(symbol) = TradableProduct::from_str(coin) {
            return self.symbol_to_id.read().await.get(&symbol).copied();
        }
        
        // Try creating symbol from coin
        let symbol = TradableProduct::from_hyperliquid_coin(coin);
        self.symbol_to_id.read().await.get(&symbol).copied()
    }
    
    pub async fn get_market_symbol(&self, id: u32) -> Option<String> {
        // Return TradableProduct symbol format
        let symbol_to_id = self.symbol_to_id.read().await;
        for (symbol, market_id) in symbol_to_id.iter() {
            if *market_id == id {
                return Some(symbol.to_string());
            }
        }
        None
    }
    
    pub async fn get_all_markets(&self) -> HashMap<u32, String> {
        // Return TradableProduct symbol format instead of raw coin names
        let symbol_to_id = self.symbol_to_id.read().await;
        let mut result = HashMap::new();
        
        for (symbol, id) in symbol_to_id.iter() {
            result.insert(*id, symbol.to_string());
        }
        
        result
    }
    
    pub async fn is_valid_coin(&self, coin: &str) -> bool {
        self.coin_to_id.read().await.contains_key(coin)
    }
    
    pub async fn market_count(&self) -> usize {
        self.markets.read().await.len()
    }
    
    pub async fn last_update_elapsed(&self) -> std::time::Duration {
        self.last_update.read().await.elapsed()
    }
    
    /// Start a background task to refresh markets periodically
    pub fn start_refresh_task(self: Arc<Self>) {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(300)); // 5 minutes
            
            loop {
                interval.tick().await;
                
                if let Err(e) = self.refresh_markets().await {
                    error!("Failed to refresh markets: {}", e);
                } else {
                    info!("Successfully refreshed market list");
                }
            }
        });
    }
}

// Implement SymbologyService trait
#[async_trait::async_trait]
impl SymbologyService for DynamicMarketRegistry {
    async fn list_symbols(&self) -> Result<Vec<TradableProduct>> {
        Ok(self.market_info.read().await.keys().cloned().collect())
    }
    
    async fn get_product_info(&self, symbol: &TradableProduct) -> Result<Option<ProductInfo>> {
        Ok(self.market_info.read().await
            .get(symbol)
            .map(|info| info.product_info.clone()))
    }
    
    async fn get_execution_info(&self, symbol: &TradableProduct, venue: &str) -> Result<Option<ExecutionInfo>> {
        if venue != "HYPERLIQUID" {
            return Ok(None);
        }
        
        Ok(self.market_info.read().await
            .get(symbol)
            .map(|info| info.execution_info.clone()))
    }
    
    async fn search_symbols(&self, query: &str) -> Result<Vec<TradableProduct>> {
        let query_upper = query.to_uppercase();
        let market_info = self.market_info.read().await;
        
        Ok(market_info
            .keys()
            .filter(|symbol| symbol.base().contains(&query_upper))
            .cloned()
            .collect())
    }
    
    async fn get_market_info(&self, symbol: &TradableProduct) -> Result<Option<MarketInfo>> {
        Ok(self.market_info.read().await.get(symbol).cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_dynamic_markets() {
        let registry = Arc::new(DynamicMarketRegistry::new());
        
        // Initial refresh
        registry.refresh_markets().await.unwrap();
        
        // Check some known markets
        assert!(registry.is_valid_coin("BTC").await);
        assert!(registry.is_valid_coin("ETH").await);
        
        // Check recently added markets that break static list
        assert!(registry.is_valid_coin("TRUMP").await);
        assert!(registry.is_valid_coin("KAITO").await);
        
        let btc_id = registry.get_market_id("BTC").await;
        assert_eq!(btc_id, Some(0));
        
        let market_count = registry.market_count().await;
        println!("Total active markets: {}", market_count);
        assert!(market_count > 150); // Should have many markets
    }
}