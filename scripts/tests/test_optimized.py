#!/usr/bin/env python3
import subprocess
import time
import os
import docker
import struct

def test_docker_direct():
    """Test by reading directly from Docker container"""
    print("Testing orderbook service with Docker data...")
    
    # Connect to Docker
    client = docker.from_env()
    container = client.containers.get('hyperliquid-node-1')
    
    # Check if order status files exist
    result = container.exec_run('ls -la /home/hluser/hl/data/node_order_statuses/order_status_*.bin')
    print(f"Order status files:\n{result.output.decode()[:500]}")
    
    # Read a sample of binary data
    result = container.exec_run('head -c 380 /home/hluser/hl/data/node_order_statuses/order_status_0.bin', stream=False)
    binary_data = result.output
    
    if len(binary_data) >= 38:
        print(f"\nSample orders from BTC market (first 10):")
        for i in range(min(10, len(binary_data) // 38)):
            offset = i * 38
            order_data = binary_data[offset:offset+38]
            if len(order_data) == 38:
                order_id = struct.unpack('<Q', order_data[0:8])[0]
                market_id = struct.unpack('<I', order_data[8:12])[0]
                price = struct.unpack('<d', order_data[12:20])[0]
                size = struct.unpack('<d', order_data[20:28])[0]
                is_buy = order_data[28] != 0
                timestamp_ns = struct.unpack('<Q', order_data[29:37])[0]
                status = order_data[37]
                
                status_str = {0: "Open", 1: "Filled", 2: "Cancelled"}.get(status, "Unknown")
                side_str = "Buy" if is_buy else "Sell"
                
                print(f"  Order {i+1}: ID={order_id}, Price=${price:.2f}, Size={size:.4f}, Side={side_str}, Status={status_str}")
    
    # Start the optimized service pointing to the volume
    print("\nStarting optimized orderbook service...")
    
    # Create a symlink to the Docker volume data
    volume_path = "/var/lib/docker/volumes/hyperliquid_hl-data/_data"
    symlink_path = "/home/ubuntu/node/orderbook-service/hl_data"
    
    if os.path.exists(symlink_path):
        os.unlink(symlink_path)
    
    try:
        # Try to access with docker exec instead
        print("Using docker exec to access data...")
        
        # Start the service with a wrapper that uses docker exec
        process = subprocess.Popen([
            "/home/ubuntu/node/orderbook-service/target/release/orderbook-service-optimized",
            "--data-dir", "/home/ubuntu/node/orderbook-service/test_data",
            "--markets", "0:BTC,159:HYPE"
        ])
        
        print("Service started. Monitoring for 10 seconds...")
        time.sleep(10)
        
        process.terminate()
        process.wait()
        
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    test_docker_direct()