# Mark Price Implementation Comparison

## Our Initial Implementation vs Hyperliquid's Exact Method

### Our Initial Approach
We implemented a generic mark price calculation based on common industry practices:

```
Mark Price = Weighted Average of:
- Simple Mid Price (bid + ask) / 2
- Impact Price (price to buy/sell $10k-50k)
- EMA Price (10 second smoothing)

With confidence-based weighting and ±50bps deviation limits
```

### Hyperliquid's Exact Method
From their documentation, they use a more sophisticated approach:

```
Mark Price = Median of:
1. Oracle price + 150s EMA(Hyperliquid mid - oracle)
2. Median(best bid, best ask, last trade) on Hyperliquid
3. Weighted median of CEX perps (Binance:3, OKX:2, Bybit:2, Gate:1, MEXC:1)

If only 2 inputs exist, add 30s EMA of Hyperliquid mid
```

### Key Differences

| Aspect | Our Implementation | Hyperliquid's Method |
|--------|-------------------|---------------------|
| **Core Algorithm** | Weighted average | Median of multiple sources |
| **External Data** | None | CEX prices + Oracle |
| **EMA Period** | 10 seconds | 150s for basis, 30s for fallback |
| **Price Sources** | Only internal book | Internal + 5 external CEXs |
| **Robustness** | Confidence-based | Median-based with fallback |
| **Impact Calculation** | Yes ($10-50k) | No |
| **Manipulation Resistance** | Deviation limits | Multiple external sources |

### Why Hyperliquid's Method is Superior

1. **External Price Anchoring**: Uses oracle and CEX prices to prevent manipulation
2. **Median vs Average**: More robust to outliers
3. **Longer EMA**: 150s smoothing reduces noise while tracking trends
4. **Multi-Source Validation**: Combines internal and external data
5. **Weighted CEX Data**: Prioritizes more liquid exchanges

### Implementation Requirements

To implement Hyperliquid's exact method, we would need:

1. **Oracle Price Feed**: External price source updated every ~3 seconds
2. **CEX Price Feeds**: Real-time perp prices from 5 exchanges
3. **Trade History**: Track last trade price (not just orderbook)
4. **Dual EMA System**: 150s for basis, 30s for fallback
5. **Weighted Median Calculator**: Handle different weights for CEXs

Our current implementation provides a reasonable approximation using only internal orderbook data, but Hyperliquid's method is significantly more robust for production use.