#!/usr/bin/env python3
import struct
import random
import time
import os

def generate_test_orders(market_id, symbol, num_orders=1000):
    """Generate test order data for a market"""
    
    # Price ranges for different markets
    price_ranges = {
        0: (90000, 110000),   # BTC
        1: (3000, 4000),      # ETH
        2: (0.8, 1.5),        # ARB
        3: (1.5, 2.5),        # OP
        4: (0.7, 1.2),        # MATIC
        5: (30, 50),          # AVAX
        6: (80, 120),         # SOL
        7: (8, 12),           # ATOM
        8: (0.3, 0.6),        # FTM
        9: (3, 5),            # NEAR
    }
    
    price_min, price_max = price_ranges.get(market_id, (10, 100))
    
    orders = []
    timestamp_ns = int(time.time() * 1e9)
    
    for i in range(num_orders):
        order_id = 1000000 + market_id * 100000 + i
        price = random.uniform(price_min, price_max)
        size = random.uniform(0.01, 10.0)
        is_buy = random.choice([True, False])
        status = random.choices([0, 1, 2], weights=[0.7, 0.2, 0.1])[0]  # 70% open, 20% filled, 10% cancelled
        
        # Pack order data (38 bytes)
        order_data = struct.pack(
            '<QIddb Q b',
            order_id,           # u64
            market_id,          # u32
            price,              # f64
            size,               # f64
            1 if is_buy else 0, # u8
            timestamp_ns + i * 1000000,  # u64 (1ms apart)
            status              # u8
        )
        
        orders.append(order_data)
    
    return b''.join(orders)

def main():
    """Generate test data for all markets"""
    markets = {
        0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC",
        5: "AVAX", 6: "SOL", 7: "ATOM", 8: "FTM", 9: "NEAR"
    }
    
    # Create directory
    os.makedirs("/home/ubuntu/node/orderbook-service/test_data/node_order_statuses", exist_ok=True)
    
    for market_id, symbol in markets.items():
        # Generate different amounts of orders to simulate real activity
        num_orders = random.randint(500, 2000)
        
        filename = f"/home/ubuntu/node/orderbook-service/test_data/node_order_statuses/order_status_{market_id}.bin"
        data = generate_test_orders(market_id, symbol, num_orders)
        
        with open(filename, 'wb') as f:
            f.write(data)
        
        print(f"Generated {num_orders} orders for {symbol} (market {market_id}): {len(data)} bytes")

if __name__ == "__main__":
    main()