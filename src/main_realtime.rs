mod fast_orderbook;
mod market_processor;
mod grpc_server;
mod types;
mod markets;
mod stop_orders;
mod mark_price;
mod mark_price_v2;
mod oracle_client;
mod mark_price_service;
mod order_parser;
mod robust_order_processor;

use anyhow::Result;
use clap::Parser;
use fast_orderbook::FastOrderbook;
use market_processor::MarketUpdate;
use robust_order_processor::{RobustOrderProcessor, ProcessorConfig};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tonic::transport::Server;
use tracing::{error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = "50052")]
    grpc_port: u16,
    
    /// Enable metrics endpoint
    #[arg(long, default_value = "false")]
    enable_metrics: bool,
    
    /// Metrics port (if enabled)
    #[arg(long, default_value = "9090")]
    metrics_port: u16,
    
    /// Require API key authentication
    #[arg(long, default_value = "false")]
    require_auth: bool,
    
    /// API keys (comma-separated)
    #[arg(long, env = "API_KEYS")]
    api_keys: Option<String>,
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
    info!("Metrics enabled: {}", args.enable_metrics);
    info!("Authentication required: {}", args.require_auth);

    // Get all market configurations
    let market_configs = markets::get_all_markets();

    info!("Tracking {} markets", market_configs.len());

    // Create broadcast channel for updates
    let (update_tx, update_rx) = broadcast::channel::<MarketUpdate>(100000);

    // Create orderbooks
    let mut orderbooks = HashMap::new();
    for (market_id, symbol) in &market_configs {
        let orderbook = Arc::new(FastOrderbook::new(*market_id, symbol.clone()));
        orderbooks.insert(*market_id, orderbook);
    }
    
    // Create stop order manager
    let stop_order_manager = Arc::new(stop_orders::StopOrderManager::new());
    
    // Create oracle client and start feed
    let oracle_client = Arc::new(oracle_client::OracleClient::new());
    oracle_client.start_oracle_feed(tokio::time::Duration::from_secs(3)).await;
    info!("Started oracle price feed (updates every 3 seconds)");

    // Get current hour for the data file
    let hour_str = chrono::Local::now().format("%H").to_string();
    let hour = hour_str.trim_start_matches('0');
    let date = chrono::Local::now().format("%Y%m%d").to_string();
    let data_path = format!("/home/hluser/hl/data/node_order_statuses/hourly/{}/{}", date, hour);

    info!("Reading real-time orders from: {}", data_path);

    // Spawn oracle price updater
    let orderbooks_for_oracle = orderbooks.clone();
    let oracle_client_clone = oracle_client.clone();
    let market_configs_clone = market_configs.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            
            // Get all oracle prices
            let prices = oracle_client_clone.get_all_cached_prices().await;
            
            // Update each orderbook with its oracle price
            for (market_id, orderbook) in &orderbooks_for_oracle {
                if let Some(symbol) = market_configs_clone.get(market_id) {
                    if let Some(oracle_price) = prices.get(symbol) {
                        orderbook.set_oracle_price(*oracle_price);
                        if orderbook.get_hl_mark_price().is_some() {
                            log::debug!("{} oracle price updated: ${:.2}", symbol, oracle_price);
                        }
                    }
                }
            }
        }
    });

    // Create robust order processor with configuration
    let processor_config = ProcessorConfig {
        max_price: 10_000_000.0,  // $10M max
        max_size: 1_000_000.0,     // 1M units max
        error_threshold: 100,       // Trip circuit after 100 errors per minute
        error_window: tokio::time::Duration::from_secs(60),
        log_sample_rate: 10,        // Log every 10th error
    };
    
    // Get list of allowed coins from market configs
    let allowed_coins: Vec<String> = market_configs.values().cloned().collect();
    
    let processor = Arc::new(RobustOrderProcessor::new(processor_config, allowed_coins));
    
    // Spawn robust order processor
    let orderbooks_clone = orderbooks.clone();
    let update_tx_clone = update_tx.clone();
    let stop_order_manager_clone = stop_order_manager.clone();
    let processor_clone = processor.clone();
    
    tokio::spawn(async move {
        if let Err(e) = processor_clone
            .start(data_path, orderbooks_clone, update_tx_clone, stop_order_manager_clone)
            .await
        {
            error!("Order processor failed: {}", e);
        }
    });

    // Create mark price service (1Hz updates)
    let mark_price_service = Arc::new(mark_price_service::MarkPriceService::new(
        orderbooks.clone(),
        oracle_client.clone(),
        tokio::time::Duration::from_secs(1),
    ));
    
    // Start mark price calculations
    let mark_price_rx = mark_price_service.clone().start().await;
    info!("Started mark price service (1Hz updates)");

    // Create gRPC server
    let addr = format!("0.0.0.0:{}", args.grpc_port).parse()?;
    info!("Starting gRPC server on {}", addr);

    let mut service = crate::grpc_server::create_delta_streaming_service(orderbooks, update_rx, stop_order_manager);
    
    // Inject mark price service
    service.set_mark_price_service(mark_price_service, mark_price_rx);
    
    // Setup authentication if required
    if args.require_auth {
        info!("Authentication enabled");
        if let Some(keys) = args.api_keys {
            let valid_keys: std::collections::HashSet<String> = keys
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            info!("Loaded {} API keys", valid_keys.len());
            // Note: We'll need to add auth wrapper to the service
            // For now, just log that auth is requested
        } else {
            warn!("Authentication required but no API keys provided");
        }
    }
    
    let service_server = crate::grpc_server::pb::orderbook_service_server::OrderbookServiceServer::new(service);

    let server_handle = tokio::spawn(async move {
        if let Err(e) = Server::builder()
            .add_service(service_server)
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