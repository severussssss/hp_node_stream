use orderbook_service::proto::orderbook_service_client::OrderbookServiceClient;
use orderbook_service::proto::SubscribeRequest;
use tokio_stream::StreamExt;
use tonic::Request;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Connecting to orderbook service...");
    
    let mut client = OrderbookServiceClient::connect("http://127.0.0.1:50051").await?;
    
    println!("Connected! Subscribing to BTC (market_id: 0) orderbook...");
    
    let request = Request::new(SubscribeRequest {
        market_ids: vec![0], // BTC market
        depth: 10, // Top 10 levels
        update_interval_ms: 0, // Real-time updates
    });
    
    let mut stream = client.subscribe_orderbook(request).await?.into_inner();
    
    println!("Subscribed! Waiting for orderbook updates...");
    
    while let Some(result) = stream.next().await {
        match result {
            Ok(snapshot) => {
                println!("\n--- Orderbook Snapshot ---");
                println!("Market: {} (ID: {})", snapshot.symbol, snapshot.market_id);
                println!("Timestamp: {} us", snapshot.timestamp_us);
                println!("Sequence: {}", snapshot.sequence);
                println!("Bids: {} levels", snapshot.bids.len());
                for (i, bid) in snapshot.bids.iter().take(5).enumerate() {
                    println!("  [{}] Price: {}, Quantity: {}, Orders: {}", 
                        i, bid.price, bid.quantity, bid.order_count);
                }
                println!("Asks: {} levels", snapshot.asks.len());
                for (i, ask) in snapshot.asks.iter().take(5).enumerate() {
                    println!("  [{}] Price: {}, Quantity: {}, Orders: {}", 
                        i, ask.price, ask.quantity, ask.order_count);
                }
            }
            Err(e) => {
                eprintln!("Error receiving update: {}", e);
                break;
            }
        }
    }
    
    Ok(())
}