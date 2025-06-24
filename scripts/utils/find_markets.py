#!/usr/bin/env python3
import json

# Common perpetual markets on Hyperliquid
POPULAR_MARKETS = {
    0: "BTC",
    1: "ETH", 
    2: "ARB",
    3: "OP",
    4: "MATIC",
    5: "AVAX",
    6: "SOL",
    7: "ATOM",
    8: "FTM",
    9: "NEAR",
    10: "GMX",
    11: "STX",
    12: "APE",
    13: "BNB",
    14: "CRV",
    15: "LTC",
    16: "DOGE",
    17: "WLD",
    159: "HYPE"
}

# Let's use the first 10 markets plus HYPE
SELECTED_MARKETS = {
    0: "BTC",
    1: "ETH",
    2: "ARB", 
    3: "OP",
    4: "MATIC",
    5: "AVAX",
    6: "SOL",
    7: "ATOM",
    8: "FTM",
    9: "NEAR",
}

print("Selected markets for testing:")
for market_id, symbol in SELECTED_MARKETS.items():
    print(f"  Market {market_id}: {symbol}")

# Generate command line args
market_args = ",".join([f"{mid}:{sym}" for mid, sym in SELECTED_MARKETS.items()])
print(f"\nCommand line argument: --markets {market_args}")