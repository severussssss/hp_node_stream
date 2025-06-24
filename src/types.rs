use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderStatusUpdate {
    pub time: String,
    pub user: String,
    pub status: String,
    pub order: OrderInfo,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderInfo {
    pub coin: String,
    pub side: String,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    pub trigger_condition: String,
    pub is_trigger: bool,
    pub trigger_px: String,
    pub children: Vec<serde_json::Value>,
    pub is_position_tpsl: bool,
    pub reduce_only: bool,
    pub order_type: String,
    pub orig_sz: String,
    pub tif: Option<String>,
    pub cloid: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderStatus {
    pub asset: u32,
    pub oid: u64,
    pub px: String,
    pub sz: String,
    pub limit_px: String,
    pub timestamp: u64,
    pub orig_sz: String,
    pub is_buy: bool,
    pub reduce_only: bool,
    pub order_type: String,
    pub is_cancelled: bool,
    pub is_filled: bool,
    pub is_triggered: bool,
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: u64,
    pub side: Side,
    pub price: f64,
    pub size: f64,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone)]
pub struct OrderLocation {
    pub side: Side,
    pub price: f64,
    pub index: usize,
}

pub const HYPE_MARKET_ID: u32 = 159;
pub const BTC_MARKET_ID: u32 = 0;

pub fn get_market_symbol(market_id: u32) -> &'static str {
    match market_id {
        HYPE_MARKET_ID => "HYPE",
        BTC_MARKET_ID => "BTC",
        _ => "UNKNOWN",
    }
}

pub fn get_market_id_from_coin(coin: &str) -> Option<u32> {
    match coin {
        "BTC" => Some(BTC_MARKET_ID),
        "HYPE" => Some(HYPE_MARKET_ID),
        _ => None,
    }
}