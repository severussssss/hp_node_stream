mod file_monitor;
mod grpc_server;
mod market_manager;
mod orderbook;
mod types;

use anyhow::Result;
use clap::Parser;
use file_monitor::FileMonitor;
use market_manager::{MarketManager, BTC_MARKET_ID, HYPE_MARKET_ID};
use parking_lot::RwLock;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tonic::transport::Server;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "/home/ubuntu/node/hl/data")]
    data_dir: PathBuf,

    #[arg(short, long, default_value = "50051")]
    grpc_port: u16,

    #[arg(short, long, value_delimiter = ',', default_values_t = vec![BTC_MARKET_ID, HYPE_MARKET_ID])]
    markets: Vec<u32>,
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

    info!("Starting orderbook service");
    info!("Data directory: {:?}", args.data_dir);
    info!("gRPC port: {}", args.grpc_port);
    info!("Markets: {:?}", args.markets);

    // Create market manager
    let (mut market_manager, update_rx) = MarketManager::new();
    market_manager.initialize_markets(args.markets.clone());
    let market_manager = Arc::new(RwLock::new(market_manager));

    // Create channel for order status updates
    let (order_tx, mut order_rx) = mpsc::unbounded_channel();

    // Create file monitor
    let markets_set: HashSet<u32> = args.markets.into_iter().collect();
    let file_monitor = Arc::new(FileMonitor::new(
        args.data_dir.clone(),
        markets_set,
        order_tx,
    ));

    // Spawn file monitor task
    let monitor_handle = {
        let file_monitor = file_monitor.clone();
        tokio::spawn(async move {
            if let Err(e) = file_monitor.start().await {
                error!("File monitor error: {}", e);
            }
        })
    };

    // Spawn order processor task
    let processor_handle = {
        let market_manager = market_manager.clone();
        tokio::spawn(async move {
            info!("Started order processor");
            while let Some((market_id, order_status)) = order_rx.recv().await {
                let manager = market_manager.read();
                if let Err(e) = manager.process_order_status(market_id, order_status) {
                    error!("Error processing order: {}", e);
                }
            }
        })
    };

    // Create and start gRPC server
    let grpc_server = grpc_server::create_grpc_server(market_manager.clone(), update_rx);
    let addr = format!("0.0.0.0:{}", args.grpc_port).parse()?;

    info!("Starting gRPC server on {}", addr);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(grpc_server)
            .serve(addr)
            .await
        {
            error!("gRPC server error: {}", e);
        }
    });

    // Wait for all tasks
    tokio::select! {
        _ = monitor_handle => {
            error!("File monitor task exited");
        }
        _ = processor_handle => {
            error!("Order processor task exited");
        }
        _ = server_handle => {
            error!("gRPC server task exited");
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
    }

    info!("Shutting down orderbook service");
    Ok(())
}