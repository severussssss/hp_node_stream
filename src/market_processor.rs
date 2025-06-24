use crate::fast_orderbook::{FastOrderbook, Order, OrderbookDelta};
use anyhow::Result;
use memmap2::MmapOptions;
use serde::Deserialize;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

#[derive(Debug, Clone)]
pub struct MarketUpdate {
    pub market_id: u32,
    pub sequence: u64,
    pub timestamp_ns: u64,
    pub deltas: Vec<OrderbookDelta>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderStatusUpdate {
    pub time: String,
    pub user: String,
    pub status: String,
    pub order: OrderInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderInfo {
    pub coin: String,
    pub side: String,
    pub limit_px: String,
    pub sz: String,
    pub oid: u64,
    pub timestamp: u64,
    #[serde(default)]
    pub trigger_condition: Option<String>,
    #[serde(default)]
    pub is_trigger: Option<bool>,
}

pub struct MarketProcessor {
    market_id: u32,
    symbol: String,
    orderbook: Arc<FastOrderbook>,
    update_tx: broadcast::Sender<MarketUpdate>,
    file_path: PathBuf,
    last_position: u64,
    
    // Performance counters
    orders_processed: u64,
    bytes_processed: u64,
    start_time: Instant,
}

impl MarketProcessor {
    pub fn new(
        market_id: u32,
        symbol: String,
        update_tx: broadcast::Sender<MarketUpdate>,
        file_path: PathBuf,
    ) -> Self {
        let orderbook = Arc::new(FastOrderbook::new(market_id, symbol.clone()));
        
        Self {
            market_id,
            symbol,
            orderbook,
            update_tx,
            file_path,
            last_position: 0,
            orders_processed: 0,
            bytes_processed: 0,
            start_time: Instant::now(),
        }
    }
    
    pub fn orderbook(&self) -> Arc<FastOrderbook> {
        self.orderbook.clone()
    }
    
    pub async fn run(mut self) {
        info!("Starting market processor for {} ({})", self.symbol, self.market_id);
        
        // Set CPU affinity if available
        #[cfg(target_os = "linux")]
        {
            if let Err(e) = self.set_cpu_affinity() {
                warn!("Failed to set CPU affinity: {}", e);
            }
        }
        
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        let mut deltas = Vec::with_capacity(100);
        
        loop {
            interval.tick().await;
            
            // Process new orders
            if let Err(e) = self.process_updates(&mut deltas).await {
                error!("Error processing updates: {}", e);
            }
            
            // Send batched updates
            if !deltas.is_empty() {
                let update = MarketUpdate {
                    market_id: self.market_id,
                    sequence: self.orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                    timestamp_ns: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64,
                    deltas: std::mem::take(&mut deltas),
                };
                
                // Non-blocking send
                let _ = self.update_tx.send(update);
            }
            
            // Log stats every second
            if self.orders_processed % 100 == 0 && self.orders_processed > 0 {
                self.log_performance();
            }
        }
    }
    
    async fn process_updates(&mut self, deltas: &mut Vec<OrderbookDelta>) -> Result<()> {
        // Check if file is binary or JSON
        let is_binary = self.file_path.extension()
            .map(|ext| ext == "bin")
            .unwrap_or(false);
        
        if is_binary {
            self.process_binary_updates(deltas).await
        } else {
            self.process_json_updates(deltas).await
        }
    }
    
    async fn process_binary_updates(&mut self, deltas: &mut Vec<OrderbookDelta>) -> Result<()> {
        use std::io::Read;
        
        // Try memory-mapped approach first for better performance
        if let Ok(file) = OpenOptions::new().read(true).open(&self.file_path) {
            if let Ok(metadata) = file.metadata() {
                let file_size = metadata.len();
                
                // Use memory-mapped I/O for files under 100MB
                if file_size < 100_000_000 && file_size > self.last_position {
                    return self.process_binary_mmap(deltas, file_size).await;
                }
            }
        }
        
        // Fall back to regular file I/O
        let mut file = File::open(&self.file_path)?;
        file.seek(SeekFrom::Start(self.last_position))?;
        
        const ORDER_SIZE: usize = 38; // Binary format size
        let mut buffer = [0u8; ORDER_SIZE];
        let mut orders_processed = 0;
        let start = Instant::now();
        
        loop {
            // Limit processing time to maintain low latency
            if start.elapsed() > Duration::from_micros(5000) {
                break;
            }
            
            match file.read_exact(&mut buffer) {
                Ok(_) => {
                    self.last_position += ORDER_SIZE as u64;
                    self.bytes_processed += ORDER_SIZE as u64;
                    
                    // Parse binary order: order_id(8), market_id(4), price(8), size(8), is_buy(1), timestamp_ns(8), status(1)
                    let order_id = u64::from_le_bytes(buffer[0..8].try_into().unwrap());
                    let market_id = u32::from_le_bytes(buffer[8..12].try_into().unwrap());
                    
                    // Skip if not our market
                    if market_id != self.market_id {
                        continue;
                    }
                    
                    let price = f64::from_le_bytes(buffer[12..20].try_into().unwrap());
                    let size = f64::from_le_bytes(buffer[20..28].try_into().unwrap());
                    let is_buy = buffer[28] != 0;
                    let timestamp_ns = u64::from_le_bytes(buffer[29..37].try_into().unwrap());
                    let status = buffer[37];
                    
                    // Process based on status
                    let delta = match status {
                        0 => { // Open
                            let order = Order {
                                id: order_id,
                                price,
                                size,
                                timestamp: timestamp_ns / 1000, // Convert to microseconds
                            };
                            Some(self.orderbook.add_order(order, is_buy))
                        }
                        1 | 2 => { // Filled or Cancelled
                            self.orderbook.remove_order(order_id, price, is_buy)
                        }
                        _ => None,
                    };
                    
                    if let Some(d) = delta {
                        deltas.push(d);
                        self.orders_processed += 1;
                        orders_processed += 1;
                    }
                    
                    // Batch size limit
                    if orders_processed >= 100 {
                        break;
                    }
                }
                Err(_) => break, // EOF or error
            }
        }
        
        Ok(())
    }
    
    async fn process_json_updates(&mut self, deltas: &mut Vec<OrderbookDelta>) -> Result<()> {
        let file = File::open(&self.file_path)?;
        let mut reader = BufReader::new(file);
        reader.seek(SeekFrom::Start(self.last_position))?;
        
        let mut lines_processed = 0;
        let start = Instant::now();
        
        for line_result in reader.lines() {
            // Limit processing time to maintain low latency
            if start.elapsed() > Duration::from_micros(5000) {
                break;
            }
            
            let line = line_result?;
            self.last_position += line.len() as u64 + 1;
            self.bytes_processed += line.len() as u64 + 1;
            
            if line.trim().is_empty() {
                continue;
            }
            
            // Parse order update
            match serde_json::from_str::<OrderStatusUpdate>(&line) {
                Ok(update) => {
                    if update.order.coin == self.symbol {
                        if let Some(delta) = self.process_order(update) {
                            deltas.push(delta);
                            self.orders_processed += 1;
                            lines_processed += 1;
                        }
                    }
                }
                Err(_) => {
                    // Skip invalid lines silently for performance
                }
            }
            
            // Batch size limit
            if lines_processed >= 100 {
                break;
            }
        }
        
        Ok(())
    }
    
    fn process_order(&self, update: OrderStatusUpdate) -> Option<OrderbookDelta> {
        // Parse price and size
        let price = update.order.limit_px.parse::<f64>().ok()?;
        let size = update.order.sz.parse::<f64>().ok()?;
        let is_buy = update.order.side == "B";
        
        match update.status.as_str() {
            "open" => {
                // Add new order
                let order = Order {
                    id: update.order.oid,
                    price,
                    size,
                    timestamp: update.order.timestamp,
                };
                Some(self.orderbook.add_order(order, is_buy))
            }
            "filled" | "canceled" | "cancelled" => {
                // Remove order
                self.orderbook.remove_order(update.order.oid, price, is_buy)
            }
            _ => None,
        }
    }
    
    fn log_performance(&self) {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        let orders_per_sec = self.orders_processed as f64 / elapsed;
        let mb_per_sec = (self.bytes_processed as f64 / 1_048_576.0) / elapsed;
        
        info!(
            "{} processor: {} orders/sec, {:.2} MB/sec, {} total orders, {} bid levels, {} ask levels",
            self.symbol,
            orders_per_sec as u64,
            mb_per_sec,
            self.orderbook.total_orders.load(std::sync::atomic::Ordering::Relaxed),
            self.orderbook.bid_count.load(std::sync::atomic::Ordering::Relaxed),
            self.orderbook.ask_count.load(std::sync::atomic::Ordering::Relaxed),
        );
    }
    
    async fn process_binary_mmap(&mut self, deltas: &mut Vec<OrderbookDelta>, file_size: u64) -> Result<()> {
        let file = OpenOptions::new().read(true).open(&self.file_path)?;
        
        unsafe {
            let mmap = MmapOptions::new().map(&file)?;
            let data = &mmap[self.last_position as usize..file_size as usize];
            
            const ORDER_SIZE: usize = 38;
            let mut offset = 0;
            let mut orders_processed = 0;
            let start = Instant::now();
            
            while offset + ORDER_SIZE <= data.len() {
                // Limit processing time to maintain low latency
                if start.elapsed() > Duration::from_micros(5000) {
                    break;
                }
                
                let order_data = &data[offset..offset + ORDER_SIZE];
                
                // Parse binary order
                let order_id = u64::from_le_bytes(order_data[0..8].try_into().unwrap());
                let market_id = u32::from_le_bytes(order_data[8..12].try_into().unwrap());
                
                // Skip if not our market
                if market_id != self.market_id {
                    offset += ORDER_SIZE;
                    continue;
                }
                
                let price = f64::from_le_bytes(order_data[12..20].try_into().unwrap());
                let size = f64::from_le_bytes(order_data[20..28].try_into().unwrap());
                let is_buy = order_data[28] != 0;
                let timestamp_ns = u64::from_le_bytes(order_data[29..37].try_into().unwrap());
                let status = order_data[37];
                
                // Process based on status
                let delta = match status {
                    0 => { // Open
                        let order = Order {
                            id: order_id,
                            price,
                            size,
                            timestamp: timestamp_ns / 1000, // Convert to microseconds
                        };
                        Some(self.orderbook.add_order(order, is_buy))
                    }
                    1 | 2 => { // Filled or Cancelled
                        self.orderbook.remove_order(order_id, price, is_buy)
                    }
                    _ => None,
                };
                
                if let Some(d) = delta {
                    deltas.push(d);
                    self.orders_processed += 1;
                    orders_processed += 1;
                }
                
                offset += ORDER_SIZE;
                
                // Batch size limit
                if orders_processed >= 100 {
                    break;
                }
            }
            
            self.last_position += offset as u64;
            self.bytes_processed += offset as u64;
        }
        
        Ok(())
    }
    
    #[cfg(target_os = "linux")]
    fn set_cpu_affinity(&self) -> Result<()> {
        use core_affinity::CoreId;
        
        // Pin to specific CPU core based on market_id
        let core_id = CoreId { id: self.market_id as usize % num_cpus::get() };
        core_affinity::set_for_current(core_id);
        
        info!("Pinned {} processor to CPU core {}", self.symbol, core_id.id);
        Ok(())
    }
}