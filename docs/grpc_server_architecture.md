# gRPC Server Connection Architecture

## High-Level Architecture Diagram

```mermaid
graph TB
    subgraph "Data Source"
        NODE[Hyperliquid Node<br/>Docker Container]
        NODE -->|tail -f| ORDERS[Order Stream<br/>JSON Lines]
    end

    subgraph "Order Processing Layer"
        ORDERS -->|AsyncBufRead| PROCESSOR[Order Processor<br/>tokio::spawn]
        PROCESSOR -->|Parse & Filter| ORDERBOOKS[(FastOrderbook<br/>HashMap<u32, Arc>)]
        PROCESSOR -->|MarketUpdate| BROADCAST[Broadcast Channel<br/>capacity: 100k]
    end

    subgraph "gRPC Server Layer"
        BROADCAST -->|clone rx| SERVICE[DeltaStreamingService]
        SERVICE -->|Arc<RwLock>| RX1[Receiver 1]
        SERVICE -->|Arc<RwLock>| RX2[Receiver 2]
        SERVICE -->|Arc<RwLock>| RXN[Receiver N]
    end

    subgraph "Client Connections"
        CLIENT1[gRPC Client 1<br/>BTC, ETH] -->|Subscribe| HANDLER1[Stream Handler 1<br/>tokio::spawn]
        CLIENT2[gRPC Client 2<br/>SOL] -->|Subscribe| HANDLER2[Stream Handler 2<br/>tokio::spawn]
        CLIENTN[gRPC Client N<br/>Multiple Markets] -->|Subscribe| HANDLERN[Stream Handler N<br/>tokio::spawn]
    end

    subgraph "Stream Handlers"
        RX1 -->|filter markets| HANDLER1
        RX2 -->|filter markets| HANDLER2
        RXN -->|filter markets| HANDLERN
        
        HANDLER1 -->|mpsc::channel<br/>buffer: 1000| STREAM1[Response Stream 1]
        HANDLER2 -->|mpsc::channel<br/>buffer: 1000| STREAM2[Response Stream 2]
        HANDLERN -->|mpsc::channel<br/>buffer: 1000| STREAMN[Response Stream N]
    end

    STREAM1 -->|Protobuf| CLIENT1
    STREAM2 -->|Protobuf| CLIENT2
    STREAMN -->|Protobuf| CLIENTN

    style NODE fill:#f9f,stroke:#333,stroke-width:2px
    style BROADCAST fill:#bbf,stroke:#333,stroke-width:2px
    style ORDERBOOKS fill:#bfb,stroke:#333,stroke-width:2px
```

## Detailed Connection Flow

```mermaid
sequenceDiagram
    participant C as gRPC Client
    participant S as gRPC Server
    participant DS as DeltaStreamingService
    participant BC as Broadcast Channel
    participant SH as Stream Handler<br/>(tokio task)
    participant OB as FastOrderbook

    C->>S: SubscribeOrderbook(market_ids=[0,1])
    S->>DS: subscribe_orderbook(request)
    DS->>DS: Clone broadcast receiver
    DS->>DS: Create mpsc channel (1000)
    DS->>SH: tokio::spawn(handler)
    
    Note over SH: Send Initial Snapshots
    loop For each requested market
        SH->>OB: get_snapshot(50)
        OB-->>SH: (bids, asks)
        SH->>OB: calculate_hl_mark_price()
        OB-->>SH: mark_price_data
        SH->>C: OrderbookSnapshot
    end
    
    Note over SH: Stream Updates
    loop While connected
        BC-->>SH: MarketUpdate
        alt Market in subscription
            SH->>OB: get_snapshot(50)
            OB-->>SH: (bids, asks)
            SH->>C: OrderbookSnapshot
        else Market not subscribed
            SH->>SH: Skip update
        end
    end
    
    Note over C: Client disconnect
    C--xS: Connection closed
    SH->>SH: tx.send() fails
    SH->>SH: Break loop & exit
```

## Connection Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Connecting: Client connects
    Connecting --> Authenticated: TLS/Auth (if enabled)
    Authenticated --> Subscribing: SubscribeOrderbook RPC
    Subscribing --> Streaming: tokio::spawn handler
    
    Streaming --> Streaming: Receive updates
    Streaming --> Disconnecting: Client disconnect
    Streaming --> Disconnecting: Server shutdown
    Streaming --> Disconnecting: Send error
    
    Disconnecting --> [*]: Cleanup resources

    note right of Streaming
        - Filter by market_ids
        - Buffer up to 1000 messages
        - Backpressure handling
    end note
```

## Resource Management

```mermaid
graph LR
    subgraph "Shared Resources (Arc)"
        OB1[Orderbook BTC]
        OB2[Orderbook ETH]
        OB3[Orderbook SOL]
        SM[StopOrderManager]
    end
    
    subgraph "Per-Connection Resources"
        C1[Client 1<br/>2 markets]
        C2[Client 2<br/>5 markets]
        C3[Client 3<br/>ALL markets]
    end
    
    subgraph "Memory Usage"
        C1 -->|~10 MB| M1[Buffers +<br/>Channels]
        C2 -->|~25 MB| M2[Buffers +<br/>Channels]
        C3 -->|~200 MB| M3[Buffers +<br/>Channels]
    end
    
    OB1 -.->|Shared Read| C1
    OB1 -.->|Shared Read| C2
    OB1 -.->|Shared Read| C3
    
    OB2 -.->|Shared Read| C1
    OB2 -.->|Shared Read| C3
    
    OB3 -.->|Shared Read| C2
    OB3 -.->|Shared Read| C3
```

## Concurrency Model

```mermaid
graph TD
    subgraph "Main Thread"
        MAIN[main.rs<br/>tokio::main]
        MAIN -->|spawn| GRPC[gRPC Server Task]
        MAIN -->|spawn| ORDER[Order Processor Task]
        MAIN -->|spawn| ORACLE[Oracle Updater Task]
    end
    
    subgraph "Order Processing"
        ORDER -->|High CPU| PARSE[Parse JSON]
        PARSE -->|Lock-free| UPDATE[Update Orderbook]
        UPDATE -->|Send| BC[Broadcast Channel]
    end
    
    subgraph "gRPC Handler Threads"
        GRPC -->|Accept| CONN1[Connection 1]
        GRPC -->|Accept| CONN2[Connection 2]
        GRPC -->|Accept| CONNN[Connection N]
        
        CONN1 -->|spawn| TASK1[Stream Task 1]
        CONN2 -->|spawn| TASK2[Stream Task 2]
        CONNN -->|spawn| TASKN[Stream Task N]
    end
    
    BC -->|Subscribe| TASK1
    BC -->|Subscribe| TASK2
    BC -->|Subscribe| TASKN
    
    style MAIN fill:#f96,stroke:#333,stroke-width:4px
    style BC fill:#bbf,stroke:#333,stroke-width:2px
```

## Key Design Patterns

### 1. **Broadcast Fan-out Pattern**
- Single producer (order processor)
- Multiple consumers (client handlers)
- Each client gets own receiver clone
- Lagging receivers drop messages

### 2. **Shared Immutable State**
- Orderbooks wrapped in `Arc`
- Read-only access from handlers
- Updates only from order processor
- Lock-free reads

### 3. **Backpressure Handling**
```rust
// Each client has bounded channel
let (tx, rx) = mpsc::channel(1000);

// Graceful degradation
if tx.send(snapshot).await.is_err() {
    // Client can't keep up, disconnect
    break;
}
```

### 4. **Resource Isolation**
- Each connection runs in separate task
- Panic in one doesn't affect others
- Independent message buffers
- No shared mutable state

## Scalability Limits

| Component | Limit | Bottleneck |
|-----------|-------|------------|
| Broadcast Channel | ~10K receivers | Memory/CPU for cloning |
| Connections | ~50K concurrent | File descriptors |
| Markets per client | 199 (all) | Network bandwidth |
| Updates/second | ~100K | Broadcast distribution |
| Orderbook depth | Unbounded* | Memory growth |

*Currently unbounded, should be limited in production

## Performance Characteristics

```
┌─────────────────┐
│ Order Arrival   │ ~100μs  Parse JSON
│                 │ ~10μs   Update orderbook  
│                 │ ~5μs    Broadcast send
└────────┬────────┘
         │
┌────────▼────────┐
│ Client Update   │ ~50μs   Filter & serialize
│                 │ ~100μs  Network send
│                 │ ~20μs   Protobuf encode
└─────────────────┘

Total Latency: ~300μs (typical)
              ~1ms (99th percentile)
```