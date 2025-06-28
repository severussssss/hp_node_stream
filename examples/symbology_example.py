#!/usr/bin/env python3
"""
Example showing how to use the new architect-style symbology.
Format: EXCHANGE-BASE/QUOTE-TYPE
Example: HYPERLIQUID-BTC/USD-PERP
"""

import sys
sys.path.append('..')
from market_config import MARKET_IDS, ID_TO_SYMBOL, get_market_id

def main():
    print("=== Architect-Style Symbology Examples ===\n")
    
    # Example 1: Looking up market IDs
    print("1. Looking up market IDs:")
    symbols = [
        "HYPERLIQUID-BTC/USD-PERP",
        "HYPERLIQUID-ETH/USD-PERP",
        "HYPERLIQUID-SOL/USD-PERP"
    ]
    
    for symbol in symbols:
        market_id = get_market_id(symbol)
        print(f"   {symbol} -> Market ID: {market_id}")
    
    print("\n2. Backward compatibility (old format):")
    old_symbols = ["BTC", "ETH", "SOL"]
    for symbol in old_symbols:
        market_id = get_market_id(symbol)
        print(f"   {symbol} -> Market ID: {market_id}")
    
    print("\n3. Getting symbol from market ID:")
    market_ids = [0, 1, 5, 159]
    for id in market_ids:
        symbol = ID_TO_SYMBOL.get(id, "Unknown")
        print(f"   Market ID {id} -> {symbol}")
    
    print("\n4. Symbol format breakdown:")
    symbol = "HYPERLIQUID-BTC/USD-PERP"
    parts = symbol.split('-')
    print(f"   Full symbol: {symbol}")
    print(f"   Exchange: {parts[0]}")
    print(f"   Pair: {parts[1]}")
    print(f"   Type: {parts[2]}")
    
    print("\n5. Common market examples:")
    examples = [
        (0, "Bitcoin perpetual"),
        (1, "Ethereum perpetual"),
        (5, "Solana perpetual"),
        (98, "dogwifhat perpetual"),
        (159, "Hyperliquid token perpetual"),
        (174, "Trump token perpetual")
    ]
    
    for market_id, description in examples:
        symbol = ID_TO_SYMBOL.get(market_id, "Unknown")
        print(f"   {description}: {symbol}")

if __name__ == "__main__":
    main()