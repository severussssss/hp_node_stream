# Mark Price Calculation Research

## What is Mark Price?

Mark price is a reference price used in perpetual futures to:
1. Calculate unrealized PnL
2. Trigger liquidations
3. Prevent market manipulation through artificial price movements

## Common Mark Price Calculation Methods

### 1. Simple Mid Price
```
Mark Price = (Best Bid + Best Ask) / 2
```
- Pros: Simple, real-time
- Cons: Easily manipulated with thin orderbooks

### 2. Weighted Average Price
```
Mark Price = (Bid Price × Ask Size + Ask Price × Bid Size) / (Bid Size + Ask Size)
```
- Weights by liquidity on each side

### 3. Impact Price / Fair Price
Considers orderbook depth to calculate the price for executing a standard position size:
```
1. Calculate average execution price for buying X contracts
2. Calculate average execution price for selling X contracts  
3. Mark Price = (Buy Impact Price + Sell Impact Price) / 2
```

### 4. Index Price + Premium/Discount
Many exchanges use:
```
Mark Price = Index Price + EMA(Basis)
where Basis = Mid Price - Index Price
```

### 5. Robust Median Method
Takes multiple price sources and uses median:
```
Prices = [Mid Price, Impact Bid, Impact Ask, Last Trade Price]
Mark Price = Median(Prices)
```

## Hyperliquid's Approach

Based on typical perpetual futures design:
1. They likely use a combination of orderbook mid price and external index
2. May apply exponential moving average (EMA) to smooth volatility
3. Likely includes safeguards against manipulation

## Our Implementation Plan

We'll implement a multi-tier mark price calculation:

1. **Basic Mark Price**: Simple mid price
2. **Impact Mark Price**: Based on orderbook depth
3. **Smoothed Mark Price**: EMA of impact price
4. **Robust Mark Price**: Median of multiple calculations

### Parameters to Consider:
- Impact size (e.g., $10,000 notional)
- EMA period (e.g., 10 seconds)
- Maximum deviation from mid (e.g., 0.5%)
- Minimum orderbook depth requirement