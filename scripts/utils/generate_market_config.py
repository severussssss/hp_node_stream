#!/usr/bin/env python3
"""
Generate complete market configuration files for the orderbook service.
This script fetches all markets from Hyperliquid and generates configuration
files for both Python and Rust components.
"""

import requests
import json
from datetime import datetime
import os

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

def generate_python_config(active_markets):
    """Generate Python configuration file"""
    content = '''#!/usr/bin/env python3
"""
Hyperliquid market configuration for Python clients.
Auto-generated on: {timestamp}
Total active markets: {count}
"""

# Market symbol to ID mapping
MARKET_IDS = {{
{market_mapping}
}}

# Reverse mapping (ID to symbol)
ID_TO_SYMBOL = {{
{id_mapping}
}}

# Market metadata
MARKET_INFO = {{
{market_info}
}}

# Get market ID by symbol
def get_market_id(symbol):
    """Get market ID from symbol"""
    return MARKET_IDS.get(symbol)

# Get symbol by market ID
def get_symbol(market_id):
    """Get symbol from market ID"""
    return ID_TO_SYMBOL.get(market_id)

# Get all active market IDs
def get_all_market_ids():
    """Get list of all active market IDs"""
    return sorted(ID_TO_SYMBOL.keys())

# Get all active symbols
def get_all_symbols():
    """Get list of all active symbols"""
    return sorted(MARKET_IDS.keys())
'''.format(
        timestamp=datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        count=len(active_markets),
        market_mapping='\n'.join(f'    "{m["symbol"]}": {m["id"]},' for m in active_markets),
        id_mapping='\n'.join(f'    {m["id"]}: "{m["symbol"]}",' for m in active_markets),
        market_info='\n'.join(
            f'    {m["id"]}: {{"symbol": "{m["symbol"]}", "max_leverage": {m["max_leverage"]}, "sz_decimals": {m["sz_decimals"]}}},'
            for m in active_markets
        )
    )
    
    with open('market_config.py', 'w') as f:
        f.write(content)
    print("Generated market_config.py")

def generate_rust_config(active_markets):
    """Generate Rust configuration file"""
    content = '''// Hyperliquid market configuration for Rust service
// Auto-generated on: {timestamp}
// Total active markets: {count}

use std::collections::HashMap;

/// Get market ID from coin symbol
pub fn get_market_id(coin: &str) -> Option<u32> {{
    match coin {{
{match_arms}
        _ => None,
    }}
}}

/// Initialize market configurations
pub fn init_market_configs() -> HashMap<u32, String> {{
    HashMap::from([
{hashmap_entries}
    ])
}}

/// Market metadata structure
#[derive(Debug, Clone)]
pub struct MarketInfo {{
    pub id: u32,
    pub symbol: String,
    pub max_leverage: u32,
    pub sz_decimals: u32,
}}

/// Get all market information
pub fn get_all_markets() -> Vec<MarketInfo> {{
    vec![
{market_info_entries}
    ]
}}
'''.format(
        timestamp=datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
        count=len(active_markets),
        match_arms='\n'.join(f'        "{m["symbol"]}" => Some({m["id"]}),' for m in active_markets),
        hashmap_entries='\n'.join(f'        ({m["id"]}, "{m["symbol"]}".to_string()),' for m in active_markets),
        market_info_entries='\n'.join(
            f'        MarketInfo {{ id: {m["id"]}, symbol: "{m["symbol"]}".to_string(), max_leverage: {m["max_leverage"]}, sz_decimals: {m["sz_decimals"]} }},'
            for m in active_markets
        )
    )
    
    with open('market_config.rs', 'w') as f:
        f.write(content)
    print("Generated market_config.rs")

def generate_protobuf_update():
    """Generate updated protobuf configuration"""
    content = '''// Additional message for complete market information
message MarketMetadata {
    uint32 market_id = 1;
    string symbol = 2;
    bool active = 3;
    uint32 max_leverage = 4;
    uint32 sz_decimals = 5;
}

// Update GetMarketsResponse to include metadata
message GetMarketsResponseV2 {
    repeated MarketMetadata markets = 1;
}
'''
    
    with open('market_metadata.proto', 'w') as f:
        f.write(content)
    print("Generated market_metadata.proto")

def main():
    """Main function to generate all configuration files"""
    print("Fetching market data from Hyperliquid...")
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
    
    print(f"Found {len(active_markets)} active markets out of {len(data['universe'])} total markets")
    
    # Generate configuration files
    generate_python_config(active_markets)
    generate_rust_config(active_markets)
    generate_protobuf_update()
    
    # Save complete market data as JSON
    output_data = {
        'generated_at': datetime.now().isoformat(),
        'total_markets': len(data['universe']),
        'active_markets': len(active_markets),
        'markets': active_markets
    }
    
    with open('hyperliquid_markets.json', 'w') as f:
        json.dump(output_data, f, indent=2)
    print("Generated hyperliquid_markets.json")
    
    # Print summary
    print(f"\nSummary:")
    print(f"  Total markets: {len(data['universe'])}")
    print(f"  Active markets: {len(active_markets)}")
    print(f"  Delisted markets: {len(data['universe']) - len(active_markets)}")
    
    # Show example usage
    print("\nExample usage in Python:")
    print("  from market_config import get_market_id, get_all_symbols")
    print("  btc_id = get_market_id('BTC')  # Returns: 0")
    print("  all_symbols = get_all_symbols()  # Returns list of all symbols")
    
    print("\nExample usage in Rust:")
    print("  use crate::market_config::{get_market_id, init_market_configs};")
    print("  let btc_id = get_market_id(\"BTC\");  // Returns: Some(0)")
    print("  let markets = init_market_configs();  // Returns HashMap")

if __name__ == "__main__":
    main()