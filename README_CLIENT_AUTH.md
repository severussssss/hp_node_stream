# Authenticated Client Usage Guide

The `node_client.py` supports multiple authentication methods for connecting to remote orderbook servers.

## Authentication Methods

### 1. API Key Authentication

Using command line:
```bash
python node_client.py --host api.example.com --port 443 --api-key YOUR_API_KEY 0 6
```

Using environment variable:
```bash
export ORDERBOOK_API_KEY=YOUR_API_KEY
python node_client.py --host api.example.com --port 443 0 6
```

### 2. JWT Token Authentication

Using command line:
```bash
python node_client.py --host api.example.com --port 443 --jwt-token YOUR_JWT_TOKEN 0 6
```

Using environment variable:
```bash
export ORDERBOOK_JWT_TOKEN=YOUR_JWT_TOKEN
python node_client.py --host api.example.com --port 443 0 6
```

### 3. TLS with Server Verification

```bash
python node_client.py --host api.example.com --port 443 --tls-ca ca.crt 0 6
```

### 4. Mutual TLS (mTLS)

```bash
python node_client.py \
  --host api.example.com \
  --port 443 \
  --tls-cert client.crt \
  --tls-key client.key \
  --tls-ca ca.crt \
  0 6
```

### 5. Combined Authentication

You can combine multiple authentication methods:

```bash
# API Key + TLS
python node_client.py \
  --host api.example.com \
  --port 443 \
  --api-key YOUR_API_KEY \
  --tls-ca ca.crt \
  0 6

# mTLS + JWT
python node_client.py \
  --host api.example.com \
  --port 443 \
  --jwt-token YOUR_JWT_TOKEN \
  --tls-cert client.crt \
  --tls-key client.key \
  --tls-ca ca.crt \
  0 6
```

## Configuration File

Instead of command line arguments, you can use a JSON configuration file:

1. Create a config file (see `client_config.json.example`):
```json
{
  "host": "orderbook.example.com",
  "port": 443,
  "api_key": "YOUR_API_KEY_HERE",
  "tls_ca": "/path/to/ca.crt"
}
```

2. Use it with:
```bash
python node_client.py --config config.json 0 6
```

## AWS EC2 Example

For connecting from an external EC2 instance:

1. Ensure security group allows outbound traffic to port 50052 (or your custom port)
2. Get the public IP or domain of the orderbook server
3. Obtain authentication credentials from the server admin

Example with API key:
```bash
# Install dependencies
pip install -r requirements.txt

# Connect to server
python node_client.py \
  --host ec2-xx-xx-xx-xx.compute-1.amazonaws.com \
  --port 50052 \
  --api-key YOUR_API_KEY \
  0 5 11  # BTC, SOL, ARB
```

## Market IDs and Symbols

Common market IDs with architect-style symbols:
- 0: HYPERLIQUID-BTC/USD-PERP
- 1: HYPERLIQUID-ETH/USD-PERP
- 5: HYPERLIQUID-SOL/USD-PERP (note: not 6!)
- 11: HYPERLIQUID-ARB/USD-PERP (note: not 2!)
- 159: HYPERLIQUID-HYPE/USD-PERP

See `market_config.py` for the complete list of 172+ supported markets.

## Troubleshooting

### Connection Refused
- Check if the server is running and accessible
- Verify the host and port are correct
- Ensure firewall/security groups allow the connection

### Authentication Failed
- Verify your API key or JWT token is valid
- Check certificate paths are correct and files exist
- Ensure certificates are not expired

### TLS Handshake Failed
- Verify the CA certificate matches the server's certificate
- For mTLS, ensure client cert is signed by a CA the server trusts
- Check certificate validity dates

### No Data Received
- Ensure you're subscribing to valid market IDs
- Check if the markets are active (have orders)
- Verify your authentication has permission for those markets

## Security Best Practices

1. **Never hardcode credentials** - Use environment variables or config files
2. **Protect config files** - Set appropriate file permissions (600)
3. **Use TLS in production** - Always encrypt connections over public networks
4. **Rotate credentials regularly** - API keys and tokens should expire
5. **Monitor access logs** - Track who's accessing your orderbook data

## Example Setup Script

```bash
#!/bin/bash
# setup_client.sh

# Create config directory
mkdir -p ~/.orderbook

# Set secure permissions
chmod 700 ~/.orderbook

# Create config file
cat > ~/.orderbook/config.json << EOF
{
  "host": "$ORDERBOOK_HOST",
  "port": $ORDERBOOK_PORT,
  "api_key": "$ORDERBOOK_API_KEY"
}
EOF

# Secure the config file
chmod 600 ~/.orderbook/config.json

# Create alias for easy access
alias orderbook='python /path/to/node_client.py --config ~/.orderbook/config.json'

echo "Setup complete! Use 'orderbook 0 6' to stream BTC and SOL"
```