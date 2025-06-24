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
    
    print("Injecting live orders to trigger updates...")
    
    # Create a new file with current timestamp to trigger the file monitor
    timestamp = int(time.time())
    temp_file = f"/tmp/order_status_0_{timestamp}.bin"
    
    with open(temp_file, "wb") as f:
        base_time = int(time.time() * 1e9)
        
        # Create some buy orders
        for i in range(10):
            order = create_order_status(
                order_id=5000 + i,
                market_id=0,
                price=95000 - i * 50,  # 95000 down to 94550
                size=0.1 + i * 0.01,
                is_buy=True,
                timestamp_ns=base_time + i * 1000000,
                status=0  # open
            )
            f.write(order)
        
        # Create some sell orders
        for i in range(10):
            order = create_order_status(
                order_id=6000 + i,
                market_id=0,
                price=95100 + i * 50,  # 95100 up to 95550
                size=0.1 + i * 0.01,
                is_buy=False,
                timestamp_ns=base_time + (10 + i) * 1000000,
                status=0  # open
            )
            f.write(order)
    
    print(f"Created {temp_file}")
    
    # Move it to the monitored directory with sudo
    final_file = os.path.join(output_dir, f"order_status_0_{timestamp}.bin")
    os.system(f"sudo cp {temp_file} {final_file}")
    os.system(f"sudo chmod 644 {final_file}")
    print(f"Copied to {final_file}")
    
    # Clean up temp file
    os.remove(temp_file)

if __name__ == "__main__":
    main()