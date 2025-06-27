# External Access and Authentication Guide

## Current Setup
The gRPC server binds to `0.0.0.0:50053` (or configured port), making it accessible from all network interfaces.

## 1. AWS Security Group Configuration

To allow external EC2 instances to connect, configure your security group:

```bash
# Allow gRPC port from specific security group
aws ec2 authorize-security-group-ingress \
    --group-id sg-YOUR-SERVER-SG \
    --protocol tcp \
    --port 50053 \
    --source-group sg-YOUR-CLIENT-SG

# Or allow from specific IP range
aws ec2 authorize-security-group-ingress \
    --group-id sg-YOUR-SERVER-SG \
    --protocol tcp \
    --port 50053 \
    --cidr 10.0.0.0/16
```

### Recommended Security Group Rules
```yaml
Inbound Rules:
  - Type: Custom TCP
    Port: 50053
    Source: sg-client-security-group  # Preferred: Security group based
  
  # Or for testing (less secure)
  - Type: Custom TCP  
    Port: 50053
    Source: 10.0.0.0/16  # Your VPC CIDR
```

## 2. Authentication Options

### Option A: TLS/mTLS (Recommended for Production)

**1. Generate certificates:**
```bash
# Create CA key and certificate
openssl genrsa -out ca-key.pem 4096
openssl req -new -x509 -days 365 -key ca-key.pem -out ca-cert.pem \
    -subj "/C=US/ST=CA/L=SF/O=YourOrg/CN=OrderbookCA"

# Create server key and certificate
openssl genrsa -out server-key.pem 4096
openssl req -new -key server-key.pem -out server-req.pem \
    -subj "/C=US/ST=CA/L=SF/O=YourOrg/CN=orderbook.yourdomain.com"
openssl x509 -req -days 365 -in server-req.pem -CA ca-cert.pem \
    -CAkey ca-key.pem -CAcreateserial -out server-cert.pem

# Create client key and certificate (for mTLS)
openssl genrsa -out client-key.pem 4096
openssl req -new -key client-key.pem -out client-req.pem \
    -subj "/C=US/ST=CA/L=SF/O=YourOrg/CN=client1"
openssl x509 -req -days 365 -in client-req.pem -CA ca-cert.pem \
    -CAkey ca-key.pem -CAcreateserial -out client-cert.pem
```

**2. Update server code:**
```rust
// src/main_realtime.rs
use tonic::transport::{Server, Identity, ServerTlsConfig, Certificate};

// Load certificates
let cert = std::fs::read_to_string("server-cert.pem")?;
let key = std::fs::read_to_string("server-key.pem")?;
let server_identity = Identity::from_pem(cert, key);

// For mTLS, also load CA cert
let ca_cert = std::fs::read_to_string("ca-cert.pem")?;
let tls_config = ServerTlsConfig::new()
    .identity(server_identity)
    .client_ca_root(Certificate::from_pem(ca_cert)); // For mTLS

// Create server with TLS
Server::builder()
    .tls_config(tls_config)?
    .add_service(service_server)
    .serve(addr)
    .await?;
```

**3. Update client code:**
```python
# Python client with TLS
import grpc

# TLS connection
with open('ca-cert.pem', 'rb') as f:
    ca_cert = f.read()

# For mTLS, also load client cert
with open('client-cert.pem', 'rb') as f:
    client_cert = f.read()
with open('client-key.pem', 'rb') as f:
    client_key = f.read()

# Create credentials
creds = grpc.ssl_channel_credentials(
    root_certificates=ca_cert,
    private_key=client_key,  # For mTLS
    certificate_chain=client_cert  # For mTLS
)

# Connect with TLS
channel = grpc.secure_channel('orderbook.yourdomain.com:50053', creds)
```

### Option B: API Key Authentication (Simple)

**1. Create auth interceptor:**
```rust
// src/auth.rs
use tonic::{Request, Status};
use tonic::service::Interceptor;

#[derive(Clone)]
pub struct ApiKeyInterceptor {
    valid_keys: std::collections::HashSet<String>,
}

impl Interceptor for ApiKeyInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        match request.metadata().get("x-api-key") {
            Some(key) => {
                let key_str = key.to_str()
                    .map_err(|_| Status::unauthenticated("Invalid API key format"))?;
                
                if self.valid_keys.contains(key_str) {
                    Ok(request)
                } else {
                    Err(Status::unauthenticated("Invalid API key"))
                }
            }
            None => Err(Status::unauthenticated("Missing API key")),
        }
    }
}

// In main.rs
let interceptor = ApiKeyInterceptor {
    valid_keys: load_api_keys()?,
};

Server::builder()
    .add_service(
        OrderbookServiceServer::with_interceptor(service, interceptor)
    )
    .serve(addr)
    .await?;
```

**2. Client with API key:**
```python
# Python client
class ApiKeyInterceptor(grpc.UnaryUnaryClientInterceptor,
                       grpc.UnaryStreamClientInterceptor):
    def __init__(self, api_key):
        self.api_key = api_key
    
    def intercept_unary_unary(self, continuation, client_call_details, request):
        metadata = [('x-api-key', self.api_key)]
        new_details = client_call_details._replace(metadata=metadata)
        return continuation(new_details, request)
    
    def intercept_unary_stream(self, continuation, client_call_details, request):
        metadata = [('x-api-key', self.api_key)]
        new_details = client_call_details._replace(metadata=metadata)
        return continuation(new_details, request)

# Use interceptor
channel = grpc.insecure_channel('server-ip:50053')
intercept_channel = grpc.intercept_channel(channel, 
    ApiKeyInterceptor('your-api-key-here'))
stub = OrderbookServiceStub(intercept_channel)
```

### Option C: JWT Authentication (Scalable)

**1. Add JWT validation:**
```rust
// Cargo.toml
jsonwebtoken = "8.3"

// src/auth.rs
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

pub struct JwtInterceptor {
    decoding_key: DecodingKey,
}

impl Interceptor for JwtInterceptor {
    fn call(&mut self, mut request: Request<()>) -> Result<Request<()>, Status> {
        let token = request.metadata()
            .get("authorization")
            .and_then(|t| t.to_str().ok())
            .and_then(|t| t.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("Missing token"))?;
        
        let token_data = decode::<Claims>(
            token,
            &self.decoding_key,
            &Validation::new(Algorithm::HS256)
        ).map_err(|_| Status::unauthenticated("Invalid token"))?;
        
        // Add claims to request extensions
        request.extensions_mut().insert(token_data.claims);
        Ok(request)
    }
}
```

### Option D: AWS IAM Authentication (Cloud Native)

**1. Use AWS SigV4 signing:**
```python
# Python client with AWS auth
import boto3
from botocore.auth import SigV4Auth
from botocore.awsrequest import AWSRequest
import grpc

def create_aws_metadata():
    session = boto3.Session()
    credentials = session.get_credentials()
    region = session.region_name or 'us-east-1'
    
    # Create signed request
    request = AWSRequest(
        method='POST',
        url=f'https://orderbook.yourdomain.com:50053',
        headers={'Host': 'orderbook.yourdomain.com'}
    )
    
    SigV4Auth(credentials, 'orderbook', region).add_auth(request)
    
    # Extract headers as gRPC metadata
    metadata = []
    for key, value in request.headers.items():
        if key.lower().startswith('x-amz-'):
            metadata.append((key.lower(), value))
    
    return metadata

# Use with gRPC
metadata = create_aws_metadata()
stub.SubscribeOrderbook(request, metadata=metadata)
```

## 3. Network Architecture Options

### Option A: Direct Connection (Simple)
```
External EC2 ──────> Security Group ──────> Orderbook Server
                         │
                    Port 50053
```

### Option B: Load Balancer (Scalable)
```
External EC2 ──────> ALB/NLB ──────> Target Group ──────> Orderbook Servers
                        │                                    (multiple instances)
                   gRPC Support
```

**ALB Configuration for gRPC:**
```bash
aws elbv2 create-target-group \
    --name orderbook-grpc \
    --protocol HTTP \
    --protocol-version GRPC \
    --port 50053 \
    --vpc-id vpc-xxx \
    --health-check-protocol HTTP \
    --health-check-path /grpc.health.v1.Health/Check
```

### Option C: VPC Peering (Secure)
```
Client VPC ←──── VPC Peering ────→ Server VPC
     │                                  │
External EC2                    Orderbook Server
                                  (private IP)
```

### Option D: AWS PrivateLink (Most Secure)
```
Client VPC ──────> VPC Endpoint ──────> NLB ──────> Orderbook Server
                  (Private connection)
```

## 4. Client Connection Examples

### From External EC2 (Python)
```python
import grpc
import subscribe_pb2
import subscribe_pb2_grpc

# For public IP
channel = grpc.insecure_channel('54.123.45.67:50053')

# For private IP (same VPC or peered)
channel = grpc.insecure_channel('10.0.1.23:50053')

# For load balancer
channel = grpc.insecure_channel('orderbook-lb.us-east-1.elb.amazonaws.com:50053')

# With keepalive for long streams
options = [
    ('grpc.keepalive_time_ms', 10000),
    ('grpc.keepalive_timeout_ms', 5000),
    ('grpc.keepalive_permit_without_calls', True),
    ('grpc.http2.max_pings_without_data', 0),
]
channel = grpc.insecure_channel('server:50053', options=options)

stub = subscribe_pb2_grpc.OrderbookServiceStub(channel)
```

### From External EC2 (Rust)
```rust
use tonic::transport::{Channel, Endpoint};

// Configure endpoint
let endpoint = Endpoint::from_static("http://54.123.45.67:50053")
    .keep_alive_interval(Duration::from_secs(10))
    .keep_alive_timeout(Duration::from_secs(5))
    .tcp_keepalive(Some(Duration::from_secs(10)))
    .tcp_nodelay(true)
    .http2_keep_alive_interval(Duration::from_secs(10));

let channel = endpoint.connect().await?;
```

## 5. Security Best Practices

1. **Never expose unauthenticated gRPC to public internet**
2. **Use TLS in production** (at minimum)
3. **Implement rate limiting** per client
4. **Monitor and log access** patterns
5. **Use VPC/private networking** when possible
6. **Rotate credentials** regularly
7. **Implement health checks** for monitoring

## 6. Testing External Access

```bash
# From external EC2, test connectivity
nc -zv server-ip 50053

# Test with grpcurl
grpcurl -plaintext server-ip:50053 list

# Test with Python client
python3 test_external_connection.py
```

## 7. Monitoring & Observability

Add logging for external connections:
```rust
// Log client connections
info!("New connection from: {:?}", request.remote_addr());

// Track metrics
metrics.increment_counter("grpc.connections.total", &[
    ("service", "orderbook"),
    ("method", request.method()),
]);
```