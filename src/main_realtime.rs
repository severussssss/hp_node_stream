mod fast_orderbook;
mod market_processor;
mod file_monitor;
mod grpc_server;
mod market_manager;
mod orderbook;
mod types;

use anyhow::Result;
use clap::Parser;
use fast_orderbook::FastOrderbook;
use market_processor::MarketUpdate;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tonic::transport::Server;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "50052")]
    grpc_port: u16,
}

// Market name to ID mapping
fn get_market_id(coin: &str) -> Option<u32> {
    match coin {
        "BTC" => Some(0),
        "ETH" => Some(1),
        "ARB" => Some(2),
        "OP" => Some(3),
        "MATIC" => Some(4),
        "AVAX" => Some(5),
        "SOL" => Some(6),
        "ATOM" => Some(7),
        "FTM" => Some(8),
        "NEAR" => Some(9),
        "HYPE" => Some(159),
        _ => None,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    let args = Args::parse();

    info!("Starting real-time orderbook service");
    info!("gRPC port: {}", args.grpc_port);

    // Market configurations
    let market_configs = HashMap::from([
        (0, "BTC".to_string()),
        (1, "ETH".to_string()),
        (2, "ARB".to_string()),
        (3, "OP".to_string()),
        (4, "MATIC".to_string()),
        (5, "AVAX".to_string()),
        (6, "SOL".to_string()),
        (7, "ATOM".to_string()),
        (8, "FTM".to_string()),
        (9, "NEAR".to_string()),
    ]);

    info!("Tracking {} markets", market_configs.len());

    // Create broadcast channel for updates
    let (update_tx, update_rx) = broadcast::channel::<MarketUpdate>(100000);

    // Create orderbooks
    let mut orderbooks = HashMap::new();
    for (market_id, symbol) in &market_configs {
        let orderbook = Arc::new(FastOrderbook::new(*market_id, symbol.clone()));
        orderbooks.insert(*market_id, orderbook);
    }

    // Get current hour for the data file
    let hour_str = chrono::Local::now().format("%H").to_string();
    let hour = hour_str.trim_start_matches('0');
    let date = chrono::Local::now().format("%Y%m%d").to_string();
    let data_path = format!("/home/hluser/hl/data/node_order_statuses/hourly/{}/{}", date, hour);

    info!("Reading real-time orders from: {}", data_path);

    // Spawn order processor
    let orderbooks_clone = orderbooks.clone();
    let update_tx_clone = update_tx.clone();
    tokio::spawn(async move {
        // Start tailing the file using docker exec
        let mut cmd = Command::new("docker")
            .args(&["exec", "hyperliquid-node-1", "tail", "-f", &data_path])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to start docker exec");

        let stdout = cmd.stdout.take().expect("Failed to get stdout");
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        let mut order_count = 0u64;
        let start_time = std::time::Instant::now();

        while let Ok(Some(line)) = lines.next_line().await {
            // Parse JSON order
            if let Ok(order_data) = serde_json::from_str::<serde_json::Value>(&line) {
                if let Some(coin) = order_data["order"]["coin"].as_str() {
                    if let Some(market_id) = get_market_id(coin) {
                        if let Some(orderbook) = orderbooks_clone.get(&market_id) {
                            // Process the order
                            if let Some(delta) = process_json_order(&order_data, orderbook) {
                                let update = MarketUpdate {
                                    market_id,
                                    sequence: orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                                    timestamp_ns: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap()
                                        .as_nanos() as u64,
                                    deltas: vec![delta],
                                };
                                let _ = update_tx_clone.send(update);
                            }

                            order_count += 1;
                            if order_count % 1000 == 0 {
                                let elapsed = start_time.elapsed().as_secs_f64();
                                let rate = order_count as f64 / elapsed;
                                info!("Processed {} orders, {:.0} orders/sec", order_count, rate);
                            }
                        }
                    }
                }
            }
        }
    });

    // Create gRPC server
    let addr = format!("0.0.0.0:{}", args.grpc_port).parse()?;
    info!("Starting gRPC server on {}", addr);

    let service = crate::grpc_server::create_delta_streaming_service(orderbooks, update_rx);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(service)
            .serve(addr)
            .await
        {
            error!("gRPC server error: {}", e);
        }
    });

    // Wait for shutdown
    tokio::select! {
        _ = server_handle => {
            error!("gRPC server task exited");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    info!("Shutting down real-time orderbook service");
    Ok(())
}

fn process_json_order(
    order_data: &serde_json::Value,
    orderbook: &Arc<FastOrderbook>,
) -> Option<crate::fast_orderbook::OrderbookDelta> {
    use crate::fast_orderbook::Order;

    let order = &order_data["order"];
    let status = order_data["status"].as_str()?;
    
    let order_id = order["oid"].as_u64()?;
    let price = order["limitPx"].as_str()?.parse::<f64>().ok()?;
    let size = order["sz"].as_str()?.parse::<f64>().ok()?;
    let is_buy = order["side"].as_str()? == "B";
    let timestamp = order["timestamp"].as_u64()?;

    match status {
        "open" => {
            let order = Order {
                id: order_id,
                price,
                size,
                timestamp,
            };
            Some(orderbook.add_order(order, is_buy))
        }
        "filled" | "canceled" | "cancelled" => {
            orderbook.remove_order(order_id, price, is_buy)
        }
        _ => None,
    }
}