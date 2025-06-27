#!/usr/bin/env python3
"""
Fetch all market information from Hyperliquid API and generate market mappings.
This script connects to Hyperliquid's public API to get the complete list of
perpetual markets with their IDs and metadata.
"""

import requests
import json
from datetime import datetime

def fetch_hyperliquid_markets():
    """Fetch all market information from Hyperliquid API"""
    url = 'https://api.hyperliquid.xyz/info'
    headers = {'Content-Type': 'application/json'}
    payload = {'type': 'meta'}
    
    try:
        response = requests.post(url, json=payload, headers=headers, timeout=10)
        response.raise_for_status()
        return response.json()
    except Exception as e:
        print(f"Error fetching market data: {e}")
        return None

def generate_market_mappings():
    """Generate Python and Rust code for market mappings"""
    data = fetch_hyperliquid_markets()
    if not data or 'universe' not in data:
        print("Failed to fetch market data")
        return
    
    # Filter active markets only
    active_markets = []
    for i, asset in enumerate(data['universe']):
        if not asset.get('isDelisted', False):
            active_markets.append({
                'id': i,
                'symbol': asset['name'],
                'max_leverage': asset['maxLeverage'],
                'sz_decimals': asset['szDecimals']
            })
    
    print(f"Found {len(active_markets)} active markets out of {len(data['universe'])} total markets\n")
    
    # Generate Python dictionary
    print("# Python market mapping (for use in Python scripts)")
    print("# Generated on:", datetime.now().strftime("%Y-%m-%d %H:%M:%S"))
    print(f"# Total active markets: {len(active_markets)}")
    print("\nMARKET_IDS = {")
    for market in active_markets:
        print(f'    "{market["symbol"]}": {market["id"]},')
    print("}\n")
    
    # Generate reverse mapping
    print("# Reverse mapping (ID to symbol)")
    print("ID_TO_SYMBOL = {")
    for market in active_markets:
        print(f'    {market["id"]}: "{market["symbol"]}",')
    print("}\n")
    
    # Generate Rust match statement
    print("// Rust market mapping (for use in main_realtime.rs)")
    print("// Generated on:", datetime.now().strftime("%Y-%m-%d %H:%M:%S"))
    print(f"// Total active markets: {len(active_markets)}")
    print("\nfn get_market_id(coin: &str) -> Option<u32> {")
    print("    match coin {")
    for market in active_markets:
        print(f'        "{market["symbol"]}" => Some({market["id"]}),')
    print("        _ => None,")
    print("    }")
    print("}\n")
    
    # Generate Rust HashMap initialization
    print("// For market_configs HashMap in main_realtime.rs")
    print("let market_configs = HashMap::from([")
    for market in active_markets:
        print(f'    ({market["id"]}, "{market["symbol"]}".to_string()),')
    print("]);\n")
    
    # Save to JSON file for reference
    output_file = "hyperliquid_markets.json"
    with open(output_file, 'w') as f:
        json.dump({
            'timestamp': datetime.now().isoformat(),
            'total_markets': len(data['universe']),
            'active_markets': len(active_markets),
            'markets': active_markets
        }, f, indent=2)
    print(f"Market data saved to {output_file}")
    
    # Print some statistics
    print("\n# Market Statistics:")
    print(f"Total markets: {len(data['universe'])}")
    print(f"Active markets: {len(active_markets)}")
    print(f"Delisted markets: {len(data['universe']) - len(active_markets)}")
    
    # Show some popular markets
    popular = ['BTC', 'ETH', 'SOL', 'ARB', 'OP', 'AVAX', 'ATOM', 'NEAR', 'HYPE', 'WIF']
    print("\n# Popular markets:")
    for symbol in popular:
        for market in active_markets:
            if market['symbol'] == symbol:
                print(f"  {symbol}: ID={market['id']}, MaxLeverage={market['max_leverage']}x")
                break

if __name__ == "__main__":
    generate_market_mappings()