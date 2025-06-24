#!/usr/bin/env python3
import struct

def parse_binary_orders(filename):
    """Parse binary order status file"""
    ORDER_SIZE = 38
    
    with open(filename, 'rb') as f:
        data = f.read()
    
    num_orders = len(data) // ORDER_SIZE
    print(f"File: {filename}")
    print(f"File size: {len(data)} bytes")
    print(f"Number of orders: {num_orders}")
    print(f"First 10 orders:\n")
    
    for i in range(min(10, num_orders)):
        offset = i * ORDER_SIZE
        order_data = data[offset:offset+ORDER_SIZE]
        
        if len(order_data) == ORDER_SIZE:
            order_id = struct.unpack('<Q', order_data[0:8])[0]
            market_id = struct.unpack('<I', order_data[8:12])[0]
            price = struct.unpack('<d', order_data[12:20])[0]
            size = struct.unpack('<d', order_data[20:28])[0]
            is_buy = order_data[28] != 0
            timestamp_ns = struct.unpack('<Q', order_data[29:37])[0]
            status = order_data[37]
            
            status_str = {0: "Open", 1: "Filled", 2: "Cancelled"}.get(status, f"Unknown({status})")
            side_str = "Buy" if is_buy else "Sell"
            
            print(f"Order {i+1}:")
            print(f"  ID: {order_id}")
            print(f"  Market: {market_id}")
            print(f"  Price: ${price:.2f}")
            print(f"  Size: {size:.4f}")
            print(f"  Side: {side_str}")
            print(f"  Status: {status_str}")
            print(f"  Timestamp: {timestamp_ns}")
            print()

if __name__ == "__main__":
    parse_binary_orders("/home/ubuntu/node/orderbook-service/test_data/node_order_statuses/order_status_0.bin")
    print("\n" + "="*50 + "\n")
    parse_binary_orders("/home/ubuntu/node/orderbook-service/test_data/node_order_statuses/order_status_159.bin")