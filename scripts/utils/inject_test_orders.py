#!/usr/bin/env python3
import struct
import time
import os

def create_order_status(order_id, market_id, price, size, is_buy, timestamp_ns, status):
    """
    Create order status in the exact binary format expected:
    order_id: u64, market_id: u32, price: f64, size: f64, is_buy: bool, timestamp_ns: u64, status: u8
    """
    # Pack the data
    data = struct.pack('<QIddbQB',
        order_id,      # u64
        market_id,     # u32  
        price,         # f64
        size,          # f64
        is_buy,        # bool (as u8)
        timestamp_ns,  # u64
        status         # u8 (0=open, 1=filled, 2=canceled)
    )
    return data

def main():
    # Create output directory
    output_dir = "/tmp/test_orders"
    os.makedirs(output_dir, exist_ok=True)
    
    print("Creating test order files...")
    
    # Generate orders for BTC-PERP
    btc_file = os.path.join(output_dir, "order_status_0.bin")
    with open(btc_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        # Create buy orders
        for i in range(20):
            order = create_order_status(
                order_id=1000 + i,
                market_id=0,
                price=94000 + i * 100,  # 94000 to 95900
                size=0.01 * (i + 1),    # 0.01 to 0.20
                is_buy=True,
                timestamp_ns=base_time + i * 1000000,  # 1ms apart
                status=0  # open
            )
            f.write(order)
        
        # Create sell orders
        for i in range(20):
            order = create_order_status(
                order_id=2000 + i,
                market_id=0,
                price=96000 + i * 100,  # 96000 to 97900
                size=0.01 * (i + 1),    # 0.01 to 0.20
                is_buy=False,
                timestamp_ns=base_time + (20 + i) * 1000000,
                status=0  # open
            )
            f.write(order)
    
    print(f"Created {btc_file} with 40 orders")
    
    # Now copy to the actual directory
    actual_dir = "/home/ubuntu/node/hl/data/order_statuses"
    target_file = os.path.join(actual_dir, "order_status_0.bin")
    
    print(f"Copying to {target_file}...")
    os.system(f"sudo cp {btc_file} {target_file}")
    os.system(f"sudo chmod 644 {target_file}")
    
    print("Done! Orders injected.")

if __name__ == "__main__":
    main()