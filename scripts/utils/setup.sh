#!/bin/bash

set -e

echo "Setting up Hyperliquid Orderbook Service..."

# Install Rust if not present
if ! command -v cargo &> /dev/null; then
    echo "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Install system dependencies
echo "Installing system dependencies..."
sudo apt update
sudo apt install -y protobuf-compiler libprotobuf-dev build-essential pkg-config

# Build the service
echo "Building orderbook service..."
cargo build --release

# Install Python dependencies for client
echo "Installing Python dependencies..."
pip install grpcio grpcio-tools

# Generate Python protobuf code
echo "Generating Python gRPC code..."
python -m grpc_tools.protoc -I./proto --python_out=. --grpc_python_out=. ./proto/orderbook.proto

echo "Setup complete!"
echo ""
echo "To run the service:"
echo "  ./target/release/orderbook-service --data-dir /home/ubuntu/node/hl/data --grpc-port 50051 --markets 0,159"
echo ""
echo "To test with the client:"
echo "  python client_example.py"