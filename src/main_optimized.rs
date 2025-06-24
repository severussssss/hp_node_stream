mod fast_orderbook;
mod market_processor;
mod file_monitor;
mod grpc_server;
mod market_manager;
mod orderbook;
mod types;

use anyhow::Result;
use clap::Parser;
use market_processor::{MarketProcessor, MarketUpdate};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use tonic::transport::Server;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "/home/ubuntu/node/hl/data")]
    data_dir: PathBuf,

    #[arg(short, long, default_value = "50051")]
    grpc_port: u16,

    #[arg(short, long, value_delimiter = ',')]
    markets: Vec<String>,  // Format: "0:BTC,159:HYPE"
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

    info!("Starting optimized orderbook service");
    info!("Data directory: {:?}", args.data_dir);
    info!("gRPC port: {}", args.grpc_port);

    // Parse market configurations
    let mut market_configs = HashMap::new();
    for market_str in &args.markets {
        let parts: Vec<&str> = market_str.split(':').collect();
        if parts.len() == 2 {
            if let Ok(market_id) = parts[0].parse::<u32>() {
                market_configs.insert(market_id, parts[1].to_string());
            }
        }
    }

    if market_configs.is_empty() {
        // Default markets
        market_configs.insert(0, "BTC".to_string());
        market_configs.insert(159, "HYPE".to_string());
    }

    info!("Markets: {:?}", market_configs);

    // Create broadcast channel for updates
    let (update_tx, update_rx) = broadcast::channel::<MarketUpdate>(100000);

    // Create and spawn market processors
    let mut processor_handles = Vec::new();
    let mut processors = HashMap::new();

    for (market_id, symbol) in market_configs {
        // Try binary file first, then fall back to text
        let bin_path = args.data_dir.join("node_order_statuses").join(format!("order_status_{}.bin", market_id));
        let txt_path = args.data_dir.join(format!("order_statuses_{}.txt", market_id));
        
        let file_path = if bin_path.exists() {
            info!("Using binary file for market {}: {:?}", market_id, bin_path);
            bin_path
        } else if txt_path.exists() {
            info!("Using text file for market {}: {:?}", market_id, txt_path);
            txt_path
        } else {
            info!("Creating order status file for market {}: {:?}", market_id, txt_path);
            std::fs::File::create(&txt_path)?;
            txt_path
        };

        let processor = MarketProcessor::new(
            market_id,
            symbol.clone(),
            update_tx.clone(),
            file_path,
        );

        let orderbook = processor.orderbook();
        processors.insert(market_id, orderbook);

        let handle = tokio::spawn(async move {
            processor.run().await;
        });

        processor_handles.push(handle);
        info!("Started processor for market {} ({})", market_id, symbol);
    }

    // Create gRPC server with delta streaming
    let addr = format!("0.0.0.0:{}", args.grpc_port).parse()?;
    info!("Starting gRPC server on {}", addr);

    // Create delta streaming service
    let service = crate::grpc_server::create_delta_streaming_service(processors, update_rx);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(service)
            .serve(addr)
            .await
        {
            error!("gRPC server error: {}", e);
        }
    });

    // Wait for all tasks
    tokio::select! {
        _ = futures::future::join_all(processor_handles) => {
            error!("Market processor tasks exited");
        }
        _ = server_handle => {
            error!("gRPC server task exited");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    info!("Shutting down optimized orderbook service");
    Ok(())
}