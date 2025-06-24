#!/usr/bin/env python3
import struct
import random
import time
import threading
import os

class OrderGenerator:
    def __init__(self, market_id, symbol, filename):
        self.market_id = market_id
        self.symbol = symbol
        self.filename = filename
        self.order_id_counter = 2000000 + market_id * 100000
        self.running = True
        
        # Price ranges
        self.price_ranges = {
            0: (95000, 105000),   # BTC
            1: (3000, 3500),      # ETH
            2: (0.9, 1.2),        # ARB
            3: (1.6, 2.2),        # OP
            4: (0.8, 1.0),        # MATIC
            5: (35, 42),          # AVAX
            6: (90, 110),         # SOL
            7: (9, 11),           # ATOM
            8: (0.35, 0.5),       # FTM
            9: (3.5, 4.5),        # NEAR
        }
        
    def generate_order(self):
        """Generate a single order"""
        price_min, price_max = self.price_ranges.get(self.market_id, (10, 100))
        
        self.order_id_counter += 1
        order_id = self.order_id_counter
        price = random.uniform(price_min, price_max)
        size = random.uniform(0.01, 5.0)
        is_buy = random.choice([True, False])
        status = 0  # Always open for new orders
        timestamp_ns = int(time.time() * 1e9)
        
        # Pack order data (38 bytes)
        return struct.pack(
            '<QIddb Q b',
            order_id,           # u64
            self.market_id,     # u32
            price,              # f64
            size,               # f64
            1 if is_buy else 0, # u8
            timestamp_ns,       # u64
            status              # u8
        )
    
    def run(self):
        """Generate orders continuously"""
        print(f"Starting order generator for {self.symbol} (market {self.market_id})")
        
        while self.running:
            # Generate batch of orders
            batch_size = random.randint(1, 10)
            
            with open(self.filename, 'ab') as f:
                for _ in range(batch_size):
                    order_data = self.generate_order()
                    f.write(order_data)
            
            # Random delay between batches (simulate real trading)
            delay = random.uniform(0.01, 0.1)  # 10-100ms
            time.sleep(delay)
    
    def stop(self):
        self.running = False

def main():
    """Run order generators for all markets"""
    markets = {
        0: "BTC", 1: "ETH", 2: "ARB", 3: "OP", 4: "MATIC",
        5: "AVAX", 6: "SOL", 7: "ATOM", 8: "FTM", 9: "NEAR"
    }
    
    generators = []
    threads = []
    
    # Create generators
    for market_id, symbol in markets.items():
        filename = f"/home/ubuntu/node/orderbook-service/test_data/node_order_statuses/order_status_{market_id}.bin"
        generator = OrderGenerator(market_id, symbol, filename)
        generators.append(generator)
        
        thread = threading.Thread(target=generator.run)
        thread.daemon = True
        threads.append(thread)
    
    # Start all threads
    for thread in threads:
        thread.start()
    
    print(f"Generating live orders for {len(markets)} markets...")
    print("Press Ctrl+C to stop")
    
    try:
        while True:
            time.sleep(1)
            # Could add stats here
    except KeyboardInterrupt:
        print("\nStopping generators...")
        for gen in generators:
            gen.stop()
        for thread in threads:
            thread.join(timeout=1)

if __name__ == "__main__":
    main()