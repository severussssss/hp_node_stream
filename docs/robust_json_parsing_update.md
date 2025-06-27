# Robust JSON Parsing Implementation

## Overview

We've integrated a robust JSON parsing solution to replace the unsafe parsing in `main_realtime.rs`. The new implementation provides:

1. **Structured Deserialization** - Using Serde for type-safe parsing
2. **Comprehensive Validation** - Price, size, and coin validation
3. **Error Recovery** - Circuit breaker pattern for fault tolerance
4. **Metrics & Monitoring** - Track parse failures and success rates

## Key Components

### 1. `order_parser.rs`
- Structured order message deserialization
- Flexible price/size parsing (handles both string and number formats)
- Validation with configurable limits
- Atomic metrics tracking

### 2. `robust_order_processor.rs`
- Circuit breaker for error recovery
- Error buffering for debugging
- Configurable error thresholds
- Monitoring task for stats reporting

## Configuration

The processor can be configured with:

```rust
ProcessorConfig {
    max_price: 10_000_000.0,     // $10M max
    max_size: 1_000_000.0,       // 1M units max
    error_threshold: 100,         // Circuit breaker threshold
    error_window: Duration::from_secs(60),
    log_sample_rate: 10,          // Log every 10th error
}
```

## Command Line Options

New options added to `main_realtime.rs`:

```bash
# Enable metrics endpoint
--enable-metrics --metrics-port 9090

# Enable API key authentication
--require-auth --api-keys "key1,key2,key3"

# Or use environment variable
export API_KEYS="key1,key2,key3"
```

## Benefits

1. **Reliability** - No more silent failures or panics
2. **Debuggability** - Detailed error logging and metrics
3. **Performance** - Efficient parsing with validation
4. **Resilience** - Circuit breaker prevents cascading failures
5. **Monitoring** - Real-time success rate tracking

## Migration Notes

The old `process_json_order` function has been completely replaced. All order processing now goes through the robust parser with proper error handling.

## Error Handling

The system now handles various error scenarios:

1. **Malformed JSON** - Logged with sample, counted in metrics
2. **Invalid Values** - Validation errors with specific reasons
3. **Unknown Markets** - Gracefully skipped with warning
4. **Circuit Breaker** - Temporary processing suspension on high error rates

## Monitoring

The processor logs statistics every 60 seconds:

```
Parser stats - Total: 100000, Parse errors: 5, Validation errors: 10, Success rate: 99.985%, Circuit: CLOSED
```

Alerts are triggered if success rate drops below 95%.