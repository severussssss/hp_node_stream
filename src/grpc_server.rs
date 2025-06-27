use crate::fast_orderbook::FastOrderbook;
use crate::market_processor::MarketUpdate;
use crate::stop_orders::StopOrderManager;
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
    Empty as GetMarketsRequest, MarketsResponse as GetMarketsResponse, GetOrderbookRequest, Market,
    OrderbookSnapshot as PbOrderbookSnapshot, Level, SubscribeRequest,
    StopOrdersRequest, StopOrdersResponse, StopOrder as PbStopOrder, RankedStopOrder as PbRankedStopOrder,
    HyperliquidMarkPrice as PbHLMarkPrice, CexPriceSnapshot as PbCEXPrices,
    MarkPriceSubscribeRequest, MarkPriceUpdate, GetMarkPriceRequest, MarkPriceResponse,
};


// Delta streaming service for optimized low-latency updates
pub struct DeltaStreamingService {
    orderbooks: HashMap<u32, Arc<FastOrderbook>>,
    update_rx: Arc<RwLock<broadcast::Receiver<MarketUpdate>>>,
    stop_order_manager: Arc<StopOrderManager>,
    mark_price_service: Option<Arc<crate::mark_price_service::MarkPriceService>>,
    mark_price_rx: Arc<RwLock<Option<broadcast::Receiver<crate::mark_price_service::MarkPriceUpdateEvent>>>>,
}

impl DeltaStreamingService {
    pub fn new(
        orderbooks: HashMap<u32, Arc<FastOrderbook>>,
        update_rx: broadcast::Receiver<MarketUpdate>,
        stop_order_manager: Arc<StopOrderManager>,
    ) -> Self {
        Self {
            orderbooks,
            update_rx: Arc::new(RwLock::new(update_rx)),
            stop_order_manager,
            mark_price_service: None,
            mark_price_rx: Arc::new(RwLock::new(None)),
        }
    }
    
    pub fn set_mark_price_service(
        &mut self,
        mark_price_service: Arc<crate::mark_price_service::MarkPriceService>,
        mark_price_rx: broadcast::Receiver<crate::mark_price_service::MarkPriceUpdateEvent>,
    ) {
        self.mark_price_service = Some(mark_price_service);
        *self.mark_price_rx.write() = Some(mark_price_rx);
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
                        timestamp: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_micros() as i64,
                        sequence: orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                        bids: bids
                            .into_iter()
                            .map(|(price, quantity)| Level {
                                price,
                                quantity,
                                })
                            .collect(),
                        asks: asks
                            .into_iter()
                            .map(|(price, quantity)| Level {
                                price,
                                quantity,
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
                            timestamp: (update.timestamp_ns / 1000) as i64,
                            sequence: update.sequence,
                            bids: bids
                                .into_iter()
                                .map(|(price, quantity)| Level {
                                    price,
                                    quantity,
                                        })
                                .collect(),
                            asks: asks
                                .into_iter()
                                .map(|(price, quantity)| Level {
                                    price,
                                    quantity,
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
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as i64,
                    sequence: orderbook.sequence.load(std::sync::atomic::Ordering::Relaxed),
                    bids: bids
                        .into_iter()
                        .map(|(price, quantity)| Level {
                            price,
                            quantity,
                        })
                        .collect(),
                    asks: asks
                        .into_iter()
                        .map(|(price, quantity)| Level {
                            price,
                            quantity,
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
            .map(|(market_id, orderbook)| Market {
                id: *market_id,
                symbol: orderbook.symbol.clone(),
            })
            .collect();

        Ok(Response::new(GetMarketsResponse { markets }))
    }

    async fn get_stop_orders(
        &self,
        request: Request<StopOrdersRequest>,
    ) -> Result<Response<StopOrdersResponse>, Status> {
        let req = request.into_inner();
        
        // Get base list of orders based on primary filter
        let mut orders = match req.filter {
            Some(pb::stop_orders_request::Filter::MarketId(market_id)) => {
                self.stop_order_manager.get_stop_orders_by_market(market_id)
            }
            Some(pb::stop_orders_request::Filter::User(user)) => {
                self.stop_order_manager.get_stop_orders_by_user(&user)
            }
            None => {
                self.stop_order_manager.get_all_stop_orders()
            }
        };

        // Apply additional filters
        if req.min_notional > 0.0 || req.max_notional > 0.0 {
            orders.retain(|order| {
                let notional = order.price * order.size;
                (req.min_notional == 0.0 || notional >= req.min_notional) &&
                (req.max_notional == 0.0 || notional <= req.max_notional)
            });
        }

        if !req.side.is_empty() {
            orders.retain(|order| order.side == req.side);
        }

        // If ranking is requested, collect market data and rank orders
        if req.rank_by_risk {
            // Collect current mid prices and orderbooks
            let mut mid_prices = HashMap::new();
            let mut orderbooks = HashMap::new();
            
            for order in &orders {
                if let Some(market_id) = crate::markets::get_market_id(&order.coin) {
                    if let Some(orderbook) = self.orderbooks.get(&market_id) {
                        if let Some((best_bid, best_ask)) = orderbook.get_best_bid_ask() {
                            let mid = (best_bid + best_ask) / 2.0;
                            mid_prices.insert(market_id, mid);
                            
                            // Get orderbook snapshot for slippage calculation
                            let (bids, asks) = orderbook.get_snapshot(50);
                            orderbooks.insert(market_id, (bids, asks));
                        }
                    }
                }
            }
            
            // Use default weights if not specified
            let distance_weight = if req.distance_weight > 0.0 { req.distance_weight } else { 0.6 };
            let slippage_weight = if req.slippage_weight > 0.0 { req.slippage_weight } else { 0.4 };
            
            // Rank the orders
            let ranked_orders = self.stop_order_manager.rank_stop_orders(
                orders,
                &mid_prices,
                &orderbooks,
                distance_weight,
                slippage_weight,
            );
            
            // Convert to protobuf format with ranking information
            let pb_orders: Vec<PbRankedStopOrder> = ranked_orders
                .into_iter()
                .filter_map(|ranked| {
                    let market_id = crate::markets::get_market_id(&ranked.order.coin).unwrap_or(0);
                    let current_mid = mid_prices.get(&market_id).copied().unwrap_or(0.0);
                    
                    // Apply distance filter if specified
                    if req.max_distance_from_mid_bps > 0.0 && ranked.distance_to_trigger_bps > req.max_distance_from_mid_bps {
                        return None;
                    }
                    
                    // Determine risk level
                    let risk_level = if ranked.risk_score >= 80.0 {
                        "HIGH".to_string()
                    } else if ranked.risk_score >= 50.0 {
                        "MEDIUM".to_string()
                    } else {
                        "LOW".to_string()
                    };
                    
                    Some(PbRankedStopOrder {
                        order: Some(PbStopOrder {
                            id: ranked.order.id,
                            user: ranked.order.user,
                            market_id,
                            coin: ranked.order.coin,
                            side: ranked.order.side,
                            price: ranked.order.price,
                            size: ranked.order.size,
                            trigger_condition: ranked.order.trigger_condition,
                            timestamp: ranked.order.timestamp,
                            notional: ranked.notional_value,
                            distance_from_mid_bps: ranked.distance_to_trigger_bps,
                            current_mid_price: current_mid,
                        }),
                        distance_to_trigger_bps: ranked.distance_to_trigger_bps,
                        expected_slippage_bps: ranked.expected_slippage_bps,
                        risk_score: ranked.risk_score,
                        risk_level,
                    })
                })
                .collect();
                
            Ok(Response::new(StopOrdersResponse { orders: pb_orders }))
        } else {
            // Non-ranked response - convert to simple format
            let pb_orders: Vec<PbRankedStopOrder> = orders
                .into_iter()
                .filter_map(|order| {
                    let notional = order.price * order.size;
                    
                    // Get current mid price for distance calculation
                    let market_id = crate::markets::get_market_id(&order.coin).unwrap_or(0);
                    let (current_mid, distance_bps) = if let Some(orderbook) = self.orderbooks.get(&market_id) {
                        if let Some((best_bid, best_ask)) = orderbook.get_best_bid_ask() {
                            let mid = (best_bid + best_ask) / 2.0;
                            let distance = ((order.price - mid).abs() / mid) * 10000.0;
                            (mid, distance)
                        } else {
                            (0.0, 0.0)
                        }
                    } else {
                        (0.0, 0.0)
                    };

                    // Apply distance filter if specified
                    if req.max_distance_from_mid_bps > 0.0 && distance_bps > req.max_distance_from_mid_bps {
                        return None;
                    }

                    Some(PbRankedStopOrder {
                        order: Some(PbStopOrder {
                            id: order.id,
                            user: order.user,
                            market_id,
                            coin: order.coin,
                            side: order.side,
                            price: order.price,
                            size: order.size,
                            trigger_condition: order.trigger_condition,
                            timestamp: order.timestamp,
                            notional,
                            distance_from_mid_bps: distance_bps,
                            current_mid_price: current_mid,
                        }),
                        distance_to_trigger_bps: distance_bps,
                        expected_slippage_bps: 0.0,
                        risk_score: 0.0,
                        risk_level: "UNKNOWN".to_string(),
                    })
                })
                .collect();

            Ok(Response::new(StopOrdersResponse { orders: pb_orders }))
        }
    }

    type SubscribeMarkPricesStream =
        Pin<Box<dyn Stream<Item = Result<MarkPriceUpdate, Status>> + Send>>;

    async fn subscribe_mark_prices(
        &self,
        request: Request<MarkPriceSubscribeRequest>,
    ) -> Result<Response<Self::SubscribeMarkPricesStream>, Status> {
        let req = request.into_inner();
        let requested_markets: std::collections::HashSet<u32> =
            req.market_ids.into_iter().collect();

        info!("New mark price subscription for {} markets", requested_markets.len());

        // Get mark price receiver
        let mark_price_rx = self.mark_price_rx.read();
        if mark_price_rx.is_none() {
            return Err(Status::unavailable("Mark price service not available"));
        }
        
        let mut rx = mark_price_rx.as_ref().unwrap().resubscribe();
        
        // Create channel for filtered stream
        let (tx, rx_stream) = tokio::sync::mpsc::channel(100);
        
        tokio::spawn(async move {
            while let Ok(update) = rx.recv().await {
                // Filter by requested markets
                if requested_markets.is_empty() || requested_markets.contains(&update.market_id) {
                    let pb_update = MarkPriceUpdate {
                        market_id: update.market_id,
                        symbol: update.symbol,
                        timestamp: update.timestamp,
                        hl_mark_price: Some(PbHLMarkPrice {
                            mark_price: update.hl_mark_price.mark_price,
                            oracle_adjusted: update.hl_mark_price.oracle_adjusted.unwrap_or(0.0),
                            internal_median: update.hl_mark_price.internal_median,
                            cex_median: update.hl_mark_price.cex_median.unwrap_or(0.0),
                            used_fallback: update.hl_mark_price.used_fallback,
                            oracle_price: 0.0, // TODO: Include if needed
                            last_trade: 0.0,   // TODO: Include if needed
                            cex_prices: None,   // TODO: Include if needed
                        }),
                        calculation_version: update.calculation_version,
                    };
                    
                    if tx.send(Ok(pb_update)).await.is_err() {
                        break; // Client disconnected
                    }
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(rx_stream);
        Ok(Response::new(Box::pin(stream) as Self::SubscribeMarkPricesStream))
    }

    async fn get_mark_price(
        &self,
        request: Request<GetMarkPriceRequest>,
    ) -> Result<Response<MarkPriceResponse>, Status> {
        let req = request.into_inner();
        
        if let Some(mark_price_service) = &self.mark_price_service {
            if let Some((hl_mark_price, age)) = mark_price_service.get_cached_mark_price(req.market_id) {
                let symbol = self.orderbooks.get(&req.market_id)
                    .map(|ob| ob.symbol.clone())
                    .unwrap_or_else(|| format!("Market{}", req.market_id));
                
                let response = MarkPriceResponse {
                    market_id: req.market_id,
                    symbol,
                    timestamp: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as i64,
                    hl_mark_price: Some(PbHLMarkPrice {
                        mark_price: hl_mark_price.mark_price,
                        oracle_adjusted: hl_mark_price.oracle_adjusted.unwrap_or(0.0),
                        internal_median: hl_mark_price.internal_median,
                        cex_median: hl_mark_price.cex_median.unwrap_or(0.0),
                        used_fallback: hl_mark_price.used_fallback,
                        oracle_price: 0.0,
                        last_trade: 0.0,
                        cex_prices: None,
                    }),
                    from_cache: true,
                    cache_age_ms: age.as_millis() as i64,
                };
                
                Ok(Response::new(response))
            } else {
                Err(Status::not_found(format!(
                    "No mark price available for market {}",
                    req.market_id
                )))
            }
        } else {
            Err(Status::unavailable("Mark price service not available"))
        }
    }
}

pub fn create_delta_streaming_service(
    orderbooks: HashMap<u32, Arc<FastOrderbook>>,
    update_rx: broadcast::Receiver<MarketUpdate>,
    stop_order_manager: Arc<StopOrderManager>,
) -> DeltaStreamingService {
    DeltaStreamingService::new(orderbooks, update_rx, stop_order_manager)
}