use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of financial instrument
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstrumentType {
    #[serde(rename = "PERP")]
    Perpetual,
    #[serde(rename = "SPOT")]
    Spot,
    #[serde(rename = "FUTURE")]
    Future,
}

/// Represents a tradable product in architect-style format
/// Full format: {exchange}-{base}/{quote}-{instrument_type}
/// Examples: "HYPERLIQUID-BTC/USD-PERP", "HYPERLIQUID-ETH/USD-PERP"
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TradableProduct {
    symbol: String,
    exchange: String,
    base: String,
    quote: String,
    instrument_type: InstrumentType,
}

impl TradableProduct {
    /// Create a new TradableProduct with full specification
    pub fn new(exchange: &str, base: &str, quote: &str, instrument_type: InstrumentType) -> Self {
        let symbol = format!("{}-{}/{}-{}", exchange, base, quote, 
            match instrument_type {
                InstrumentType::Perpetual => "PERP",
                InstrumentType::Spot => "SPOT",
                InstrumentType::Future => "FUTURE",
            }
        );
        
        Self {
            symbol,
            exchange: exchange.to_string(),
            base: base.to_string(),
            quote: quote.to_string(),
            instrument_type,
        }
    }
    
    /// Create from Hyperliquid coin name (assumes PERP with USD quote)
    pub fn from_hyperliquid_coin(coin: &str) -> Self {
        Self::new("HYPERLIQUID", coin, "USD", InstrumentType::Perpetual)
    }
    
    /// Parse from full architect format "EXCHANGE-BASE/QUOTE-TYPE"
    pub fn from_str(s: &str) -> Result<Self> {
        // Try to parse full format: EXCHANGE-BASE/QUOTE-TYPE
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 3 {
            let exchange = parts[0];
            
            // Middle part should contain BASE/QUOTE
            if let Some((base, quote)) = parts[1].split_once('/') {
                let instrument_type = match parts[2] {
                    "PERP" => InstrumentType::Perpetual,
                    "SPOT" => InstrumentType::Spot,
                    "FUTURE" => InstrumentType::Future,
                    _ => bail!("Unknown instrument type: {}", parts[2]),
                };
                
                return Ok(Self::new(exchange, base, quote, instrument_type));
            }
        }
        
        // Fallback: try simple format "BASE/QUOTE" and assume HYPERLIQUID-PERP
        if let Some((base, quote)) = s.split_once('/') {
            return Ok(Self::new("HYPERLIQUID", base, quote, InstrumentType::Perpetual));
        }
        
        bail!("Invalid symbol format: {}. Expected EXCHANGE-BASE/QUOTE-TYPE or BASE/QUOTE", s);
    }
    
    /// Get the base asset (what's being priced)
    pub fn base(&self) -> &str {
        &self.base
    }
    
    /// Get the quote asset (pricing currency)
    pub fn quote(&self) -> &str {
        &self.quote
    }
    
    /// Get the exchange
    pub fn exchange(&self) -> &str {
        &self.exchange
    }
    
    /// Get the full architect-style symbol
    pub fn symbol(&self) -> &str {
        &self.symbol
    }
    
    /// Get simplified symbol without exchange (BASE/QUOTE)
    pub fn simple_symbol(&self) -> String {
        format!("{}/{}", self.base, self.quote)
    }
    
    /// Get base and quote as tuple
    pub fn base_quote(&self) -> (&str, &str) {
        (self.base(), self.quote())
    }
    
    /// Convert back to Hyperliquid format (just the base)
    pub fn to_hyperliquid_coin(&self) -> &str {
        self.base()
    }
}

impl fmt::Display for TradableProduct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.symbol)
    }
}

impl fmt::Display for InstrumentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InstrumentType::Perpetual => write!(f, "PERP"),
            InstrumentType::Spot => write!(f, "SPOT"),
            InstrumentType::Future => write!(f, "FUTURE"),
        }
    }
}

/// Execution venue information (following architect pattern)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInfo {
    pub execution_venue: String,      // "HYPERLIQUID"
    pub exchange_symbol: Option<String>, // Native exchange symbol if different
    pub tick_size: f64,              // Minimum price increment
    pub step_size: f64,              // Minimum size increment
    pub min_order_quantity: f64,     // Minimum order size
    pub max_leverage: u32,           // Maximum leverage allowed
    pub is_delisted: bool,           // Whether actively traded
}

/// Product information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductInfo {
    pub product_type: String,        // "PERP" for Hyperliquid
    pub display_name: String,        // Human-readable name
    pub base_currency: String,       // Base asset
    pub quote_currency: String,      // Quote asset (USD)
    pub sz_decimals: u32,           // Size decimal precision
}

/// Complete market information combining all metadata
#[derive(Debug, Clone)]
pub struct MarketInfo {
    pub id: u32,                    // Hyperliquid market ID
    pub symbol: TradableProduct,    // Standardized symbol
    pub execution_info: ExecutionInfo,
    pub product_info: ProductInfo,
}

impl MarketInfo {
    /// Create from Hyperliquid asset info
    pub fn from_hyperliquid(
        id: u32,
        name: String,
        max_leverage: u32,
        sz_decimals: u32,
        is_delisted: bool,
    ) -> Self {
        let symbol = TradableProduct::from_hyperliquid_coin(&name);
        
        // Derive tick size from decimals (Hyperliquid specific)
        let tick_size = 10f64.powi(-(sz_decimals as i32));
        let step_size = tick_size; // Usually same as tick size
        
        Self {
            id,
            symbol: symbol.clone(),
            execution_info: ExecutionInfo {
                execution_venue: "HYPERLIQUID".to_string(),
                exchange_symbol: Some(name.clone()),
                tick_size,
                step_size,
                min_order_quantity: step_size,
                max_leverage,
                is_delisted,
            },
            product_info: ProductInfo {
                product_type: "PERP".to_string(),
                display_name: format!("{} Perpetual", name),
                base_currency: symbol.base().to_string(),
                quote_currency: symbol.quote().to_string(),
                sz_decimals,
            },
        }
    }
}

/// Symbology service interface (following architect pattern)
#[async_trait::async_trait]
pub trait SymbologyService: Send + Sync {
    /// List all available symbols
    async fn list_symbols(&self) -> Result<Vec<TradableProduct>>;
    
    /// Get detailed product information
    async fn get_product_info(&self, symbol: &TradableProduct) -> Result<Option<ProductInfo>>;
    
    /// Get execution information for a venue
    async fn get_execution_info(&self, symbol: &TradableProduct, venue: &str) -> Result<Option<ExecutionInfo>>;
    
    /// Search symbols by partial match
    async fn search_symbols(&self, query: &str) -> Result<Vec<TradableProduct>>;
    
    /// Get complete market info
    async fn get_market_info(&self, symbol: &TradableProduct) -> Result<Option<MarketInfo>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tradable_product() {
        // Test full architect format
        let btc = TradableProduct::from_hyperliquid_coin("BTC");
        assert_eq!(btc.symbol(), "HYPERLIQUID-BTC/USD-PERP");
        assert_eq!(btc.base(), "BTC");
        assert_eq!(btc.quote(), "USD");
        assert_eq!(btc.exchange(), "HYPERLIQUID");
        assert_eq!(btc.simple_symbol(), "BTC/USD");
        assert_eq!(btc.to_hyperliquid_coin(), "BTC");
        
        // Test parsing full format
        let eth = TradableProduct::from_str("HYPERLIQUID-ETH/USD-PERP").unwrap();
        assert_eq!(eth.base(), "ETH");
        assert_eq!(eth.quote(), "USD");
        assert_eq!(eth.exchange(), "HYPERLIQUID");
        
        // Test parsing simple format (backward compat)
        let sol = TradableProduct::from_str("SOL/USD").unwrap();
        assert_eq!(sol.symbol(), "HYPERLIQUID-SOL/USD-PERP");
        assert_eq!(sol.base(), "SOL");
    }
    
    #[test]
    fn test_market_info() {
        let info = MarketInfo::from_hyperliquid(
            0,
            "BTC".to_string(),
            50,
            1,
            false,
        );
        
        assert_eq!(info.symbol.symbol(), "HYPERLIQUID-BTC/USD-PERP");
        assert_eq!(info.execution_info.tick_size, 0.1);
        assert_eq!(info.execution_info.max_leverage, 50);
        assert_eq!(info.product_info.product_type, "PERP");
    }
}