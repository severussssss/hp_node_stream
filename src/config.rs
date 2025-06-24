use std::collections::HashSet;

pub struct Config {
    pub data_dir: String,
    pub grpc_port: u16,
    pub markets: HashSet<u32>,
    pub max_orders_per_level: usize,
    pub max_levels_per_side: usize,
    pub max_total_orders: usize,
}

impl Default for Config {
    fn default() -> Self {
        let mut markets = HashSet::new();
        markets.insert(0);   // BTC
        markets.insert(159); // HYPE
        
        Self {
            data_dir: "/home/ubuntu/node/hl/data".to_string(),
            grpc_port: 50051,
            markets,
            max_orders_per_level: 100,    // Max 100 orders per price level
            max_levels_per_side: 1000,    // Max 1000 price levels per side
            max_total_orders: 10000,      // Max 10k orders total per book
        }
    }
}