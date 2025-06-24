#!/usr/bin/env python3
import struct
import time
import os

def create_test_order(market_id, order_id, price, size, is_buy, timestamp_us=None):
    """Create a test order in the format expected by the orderbook service"""
    if timestamp_us is None:
        timestamp_us = int(time.time() * 1_000_000)
    
    # Pack order data: market_id (u32), order_id (u64), price (f64), size (f64), is_buy (u8), timestamp_us (u64)
    order_data = struct.pack('<IQddBQ', 
                           market_id,
                           order_id, 
                           price,
                           size,
                           1 if is_buy else 0,
                           timestamp_us)
    return order_data

def main():
    # Create test directory if it doesn't exist
    test_dir = "/home/ubuntu/node/hl/data/order_statuses"
    os.makedirs(test_dir, exist_ok=True)
    
    # Create test orders for BTC-PERP (market_id=0)
    with open(f"{test_dir}/order_status_0.bin", "wb") as f:
        # Create some buy orders
        for i in range(10):
            order = create_test_order(
                market_id=0,
                order_id=1000 + i,
                price=95000 - i * 100,  # Prices from 95000 down
                size=0.1 + i * 0.05,
                is_buy=True
            )
            f.write(order)
        
        # Create some sell orders
        for i in range(10):
            order = create_test_order(
                market_id=0,
                order_id=2000 + i,
                price=95100 + i * 100,  # Prices from 95100 up
                size=0.1 + i * 0.05,
                is_buy=False
            )
            f.write(order)
    
    print("Created test orders for BTC-PERP")
    
    # Create test orders for HYPE-PERP (market_id=159)
    with open(f"{test_dir}/order_status_159.bin", "wb") as f:
        # Create some orders
        for i in range(5):
            order = create_test_order(
                market_id=159,
                order_id=3000 + i,
                price=25.0 - i * 0.1,
                size=100 + i * 50,
                is_buy=True
            )
            f.write(order)
        
        for i in range(5):
            order = create_test_order(
                market_id=159,
                order_id=4000 + i,
                price=25.1 + i * 0.1,
                size=100 + i * 50,
                is_buy=False
            )
            f.write(order)
    
    print("Created test orders for HYPE-PERP")

if __name__ == "__main__":
    main()