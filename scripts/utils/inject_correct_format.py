#!/usr/bin/env python3
import struct
import time
import os

def create_order_status(order_id, market_id, price, size, is_buy, timestamp_ns, status):
    """Create order status in binary format"""
    data = struct.pack('<QIddbQB',
        order_id, market_id, price, size, is_buy, timestamp_ns, status
    )
    return data

def main():
    output_dir = "/home/ubuntu/node/hl/data/order_statuses"
    
    print("Creating orders with correct file naming...")
    
    # Create files with numeric names for market 0 (BTC)
    temp_file = "/tmp/0.bin"
    
    with open(temp_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        # Create buy orders with good spread
        for i in range(10):
            order = create_order_status(
                order_id=10000 + i,
                market_id=0,
                price=94900 - i * 10,  # 94900 down to 94810
                size=0.5 + i * 0.1,
                is_buy=True,
                timestamp_ns=base_time + i * 1000000,
                status=0  # open
            )
            f.write(order)
        
        # Create sell orders
        for i in range(10):
            order = create_order_status(
                order_id=20000 + i,
                market_id=0,
                price=95000 + i * 10,  # 95000 up to 95090
                size=0.5 + i * 0.1,
                is_buy=False,
                timestamp_ns=base_time + (10 + i) * 1000000,
                status=0  # open
            )
            f.write(order)
    
    # Copy to monitored directory
    final_file = os.path.join(output_dir, "0.bin")
    os.system(f"sudo cp {temp_file} {final_file}")
    os.system(f"sudo chmod 644 {final_file}")
    print(f"Created {final_file} with 20 orders")
    
    # Also create for market 159 (HYPE)
    temp_file = "/tmp/159.bin"
    
    with open(temp_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        for i in range(10):
            order = create_order_status(
                order_id=30000 + i,
                market_id=159,
                price=25.0 - i * 0.01,
                size=1000 + i * 100,
                is_buy=True,
                timestamp_ns=base_time + i * 1000000,
                status=0
            )
            f.write(order)
        
        for i in range(10):
            order = create_order_status(
                order_id=40000 + i,
                market_id=159,
                price=25.1 + i * 0.01,
                size=1000 + i * 100,
                is_buy=False,
                timestamp_ns=base_time + (10 + i) * 1000000,
                status=0
            )
            f.write(order)
    
    final_file = os.path.join(output_dir, "159.bin")
    os.system(f"sudo cp {temp_file} {final_file}")
    os.system(f"sudo chmod 644 {final_file}")
    print(f"Created {final_file} with 20 orders")

if __name__ == "__main__":
    main()