use crate::fast_orderbook::FastOrderbook;
use crate::market_processor::MarketUpdate;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::info;

pub mod pb {
    tonic::include_proto!("orderbook");
}

use pb::orderbook_service_server::{OrderbookService, OrderbookServiceServer};
use pb::{
    GetMarketsRequest, GetMarketsResponse, GetOrderbookRequest, MarketInfo,
    OrderbookSnapshot as PbOrderbookSnapshot, PriceLevel, SubscribeRequest,
};


// Delta streaming service for optimized low-latency updates
pub struct DeltaStreamingService {
    orderbooks: HashMap<u32, Arc<FastOrderbook>>,
    update_rx: Arc<RwLock<broadcast::Receiver<MarketUpdate>>>,
}

impl DeltaStreamingService {
    pub fn new(
        orderbooks: HashMap<u32, Arc<FastOrderbook>>,
        update_rx: broadcast::Receiver<MarketUpdate>,
    ) -> Self {
        Self {
            orderbooks,
            update_rx: Arc::new(RwLock::new(update_rx)),
        }
    }
}

#[tonic::async_trait]
impl OrderbookService for DeltaStreamingService {
    type SubscribeOrderbookStream =
        Pin<Box<dyn Stream<Item = Result<PbOrderbookSnapshot, Status>> + Send>>;

    async fn subscribe_orderbook(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeOrderbookStream>, Status> {
        let subscribe_request = request.into_inner();
        let requested_markets: std::collections::HashSet<u32> =
            subscribe_request.market_ids.into_iter().collect();

        info!("New delta subscription for markets: {:?}", requested_markets);

        // Clone the broadcast receiver
        let mut rx = self.update_rx.write().resubscribe();
        let orderbooks = self.orderbooks.clone();

        // Create a channel for the stream
        let (tx, rx_stream) = tokio::sync::mpsc::channel(1000);

        // Spawn a task to handle the stream
        tokio::spawn(async move {
            // Send initial snapshots
            for market_id in &requested_markets {
                if let Some(orderbook) = orderbooks.get(market_id) {
                    let (bids, asks) = orderbook.get_snapshot(50);
                    let snapshot = PbOrderbookSnapshot {
                        market_id: *market_id,
                        symbol: orderbook.symbol.clone(),
                        timestamp_us: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as u64,
                        sequence: orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                        bids: bids
                            .into_iter()
                            .map(|(price, quantity)| PriceLevel {
                                price,
                                quantity,
                                order_count: 0,
                            })
                            .collect(),
                        asks: asks
                            .into_iter()
                            .map(|(price, quantity)| PriceLevel {
                                price,
                                quantity,
                                order_count: 0,
                            })
                            .collect(),
                    };
                    let _ = tx.send(Ok(snapshot)).await;
                }
            }

            // Stream delta updates
            while let Ok(update) = rx.recv().await {
                if requested_markets.contains(&update.market_id) {
                    // Convert deltas to snapshot format for now
                    // In a production system, we'd have a separate delta message type
                    if let Some(orderbook) = orderbooks.get(&update.market_id) {
                        let (bids, asks) = orderbook.get_snapshot(50);
                        let snapshot = PbOrderbookSnapshot {
                            market_id: update.market_id,
                            symbol: orderbook.symbol.clone(),
                            timestamp_us: update.timestamp_ns / 1000,
                            sequence: update.sequence,
                            bids: bids
                                .into_iter()
                                .map(|(price, quantity)| PriceLevel {
                                    price,
                                    quantity,
                                    order_count: 0,
                                })
                                .collect(),
                            asks: asks
                                .into_iter()
                                .map(|(price, quantity)| PriceLevel {
                                    price,
                                    quantity,
                                    order_count: 0,
                                })
                                .collect(),
                        };
                        if tx.send(Ok(snapshot)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx_stream);
        Ok(Response::new(Box::pin(stream) as Self::SubscribeOrderbookStream))
    }

    async fn get_orderbook(
        &self,
        request: Request<GetOrderbookRequest>,
    ) -> Result<Response<PbOrderbookSnapshot>, Status> {
        let req = request.into_inner();
        let depth = req.depth as usize;

        match self.orderbooks.get(&req.market_id) {
            Some(orderbook) => {
                let (bids, asks) = orderbook.get_snapshot(depth);
                let snapshot = PbOrderbookSnapshot {
                    market_id: req.market_id,
                    symbol: orderbook.symbol.clone(),
                    timestamp_us: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64,
                    sequence: orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                    bids: bids
                        .into_iter()
                        .map(|(price, quantity)| PriceLevel {
                            price,
                            quantity,
                            order_count: 0,
                        })
                        .collect(),
                    asks: asks
                        .into_iter()
                        .map(|(price, quantity)| PriceLevel {
                            price,
                            quantity,
                            order_count: 0,
                        })
                        .collect(),
                };
                Ok(Response::new(snapshot))
            }
            None => Err(Status::not_found(format!(
                "Market {} not found",
                req.market_id
            ))),
        }
    }

    async fn get_markets(
        &self,
        _request: Request<GetMarketsRequest>,
    ) -> Result<Response<GetMarketsResponse>, Status> {
        let markets = self
            .orderbooks
            .iter()
            .map(|(market_id, orderbook)| MarketInfo {
                market_id: *market_id,
                symbol: orderbook.symbol.clone(),
                active: true,
            })
            .collect();

        Ok(Response::new(GetMarketsResponse { markets }))
    }
}

pub fn create_delta_streaming_service(
    orderbooks: HashMap<u32, Arc<FastOrderbook>>,
    update_rx: broadcast::Receiver<MarketUpdate>,
) -> OrderbookServiceServer<DeltaStreamingService> {
    let service = DeltaStreamingService::new(orderbooks, update_rx);
    OrderbookServiceServer::new(service)
}