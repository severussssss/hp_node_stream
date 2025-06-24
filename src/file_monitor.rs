use crate::types::{OrderStatus, OrderStatusUpdate, get_market_id_from_coin};
use anyhow::Result;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub struct FileMonitor {
    data_dir: PathBuf,
    markets: HashSet<u32>,
    order_tx: mpsc::UnboundedSender<(u32, OrderStatus)>,
    file_positions: dashmap::DashMap<PathBuf, u64>,
}

impl FileMonitor {
    pub fn new(
        data_dir: PathBuf,
        markets: HashSet<u32>,
        order_tx: mpsc::UnboundedSender<(u32, OrderStatus)>,
    ) -> Self {
        Self {
            data_dir,
            markets,
            order_tx,
            file_positions: dashmap::DashMap::new(),
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<()> {
        let (tx, mut rx) = mpsc::channel(100);

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    let _ = tx.blocking_send(event);
                }
            },
            Config::default(),
        )?;

        let order_status_dir = self.data_dir.join("node_order_statuses");
        
        // Check if directory exists (could be a symlink)
        if !order_status_dir.exists() && !order_status_dir.is_symlink() {
            std::fs::create_dir_all(&order_status_dir)?;
            info!("Created order_statuses directory: {:?}", order_status_dir);
        }
        
        watcher.watch(&order_status_dir, RecursiveMode::Recursive)?;

        info!("Started monitoring directory: {:?}", order_status_dir);

        // Process existing files
        self.scan_existing_files(&order_status_dir).await?;

        // Watch for new events
        while let Some(event) = rx.recv().await {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) => {
                    for path in event.paths {
                        if path.is_file() {
                            if let Err(e) = self.process_file(&path).await {
                                error!("Error processing file {:?}: {}", path, e);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    async fn scan_existing_files(&self, dir: &Path) -> Result<()> {
        self.scan_directory_recursive(dir).await
    }
    
    fn scan_directory_recursive<'a>(&'a self, dir: &'a Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        
                        if path.is_dir() {
                            // Recursively scan subdirectories
                            if let Err(e) = self.scan_directory_recursive(&path).await {
                                error!("Error scanning directory {:?}: {}", path, e);
                            }
                        } else if path.is_file() {
                            // Process files that look like order status files (numeric names or .bin files)
                            if let Some(filename) = path.file_name() {
                                if let Some(filename_str) = filename.to_str() {
                                    // Check if it's a numeric filename or ends with .bin
                                    let is_numeric = filename_str.chars().all(|c| c.is_numeric());
                                    let is_bin = filename_str.ends_with(".bin");
                                    
                                    if is_numeric || is_bin {
                                        if let Err(e) = self.process_file(&path).await {
                                            error!("Error processing existing file {:?}: {}", path, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            Ok(())
        })
    }

    async fn process_file(&self, path: &Path) -> Result<()> {
        debug!("Processing file: {:?}", path);
        
        // Check if it's a binary file
        if path.extension().and_then(|s| s.to_str()) == Some("bin") {
            return self.process_binary_file(path).await;
        }
        
        // Original JSON processing logic
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);

        // Get last read position
        let start_pos = self.file_positions.get(path).map(|v| *v).unwrap_or(0);
        reader.seek(SeekFrom::Start(start_pos))?;

        let mut current_pos = start_pos;
        let mut lines_processed = 0;

        for line in reader.lines() {
            let line = line?;
            current_pos += line.len() as u64 + 1; // +1 for newline

            if line.trim().is_empty() {
                continue;
            }

            match serde_json::from_str::<OrderStatusUpdate>(&line) {
                Ok(update) => {
                    // Convert coin name to market ID
                    if let Some(market_id) = get_market_id_from_coin(&update.order.coin) {
                        if self.markets.contains(&market_id) {
                            // Convert to OrderStatus format expected by the system
                            let order_status = OrderStatus {
                                asset: market_id,
                                oid: update.order.oid,
                                px: update.order.limit_px.clone(),
                                sz: update.order.sz.clone(),
                                limit_px: update.order.limit_px,
                                timestamp: update.order.timestamp,
                                orig_sz: update.order.orig_sz,
                                is_buy: update.order.side == "B",
                                reduce_only: update.order.reduce_only,
                                order_type: update.order.order_type,
                                is_cancelled: update.status == "cancelled",
                                is_filled: update.status == "filled",
                                is_triggered: update.order.is_trigger,
                            };
                            
                            if let Err(e) = self.order_tx.send((market_id, order_status)) {
                                error!("Failed to send order status: {}", e);
                                break;
                            }
                            lines_processed += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to parse order status from line: {} - error: {}", line, e);
                }
            }
        }
        
        if lines_processed == 0 {
            debug!("No matching orders found in file {:?}", path);
        }

        // Update file position
        self.file_positions.insert(path.to_path_buf(), current_pos);

        if lines_processed > 0 {
            debug!("Processed {} orders from {:?}", lines_processed, path);
        }

        Ok(())
    }
    
    async fn process_binary_file(&self, path: &Path) -> Result<()> {
        debug!("Processing binary file: {:?}", path);
        use std::io::Read;
        
        let mut file = File::open(path)?;
        
        // Get last read position
        let start_pos = self.file_positions.get(path).map(|v| *v).unwrap_or(0);
        file.seek(SeekFrom::Start(start_pos))?;
        
        let mut current_pos = start_pos;
        let mut orders_processed = 0;
        
        // Binary format: order_id(8), market_id(4), price(8), size(8), is_buy(1), timestamp_ns(8), status(1) = 38 bytes
        const ORDER_SIZE: usize = 38;
        let mut buffer = [0u8; ORDER_SIZE];
        
        loop {
            match file.read_exact(&mut buffer) {
                Ok(_) => {
                    // Parse binary data
                    let order_id = u64::from_le_bytes(buffer[0..8].try_into().unwrap());
                    let market_id = u32::from_le_bytes(buffer[8..12].try_into().unwrap());
                    let price = f64::from_le_bytes(buffer[12..20].try_into().unwrap());
                    let size = f64::from_le_bytes(buffer[20..28].try_into().unwrap());
                    let is_buy = buffer[28] != 0;
                    let timestamp_ns = u64::from_le_bytes(buffer[29..37].try_into().unwrap());
                    let status = buffer[37];
                    
                    current_pos += ORDER_SIZE as u64;
                    
                    // Check if this market is being monitored
                    if self.markets.contains(&market_id) {
                        // Convert to OrderStatus format
                        let order_status = OrderStatus {
                            asset: market_id,
                            oid: order_id,
                            px: price.to_string(),
                            sz: size.to_string(),
                            limit_px: price.to_string(),
                            timestamp: timestamp_ns / 1_000_000, // Convert ns to ms
                            orig_sz: size.to_string(),
                            is_buy,
                            reduce_only: false,
                            order_type: "limit".to_string(),
                            is_cancelled: status == 2,
                            is_filled: status == 1,
                            is_triggered: false,
                        };
                        
                        if let Err(e) = self.order_tx.send((market_id, order_status)) {
                            error!("Failed to send order status: {}", e);
                            break;
                        }
                        orders_processed += 1;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                    // End of file reached
                    break;
                }
                Err(e) => {
                    error!("Error reading binary file: {}", e);
                    break;
                }
            }
        }
        
        // Update file position
        self.file_positions.insert(path.to_path_buf(), current_pos);
        
        if orders_processed > 0 {
            info!("Processed {} orders from binary file {:?}", orders_processed, path);
        }
        
        Ok(())
    }
}