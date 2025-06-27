use tonic::{Request, Status};
use std::collections::HashSet;
use std::sync::Arc;
use parking_lot::RwLock;

/// Simple API key authentication interceptor
#[derive(Clone)]
pub struct ApiKeyInterceptor {
    valid_keys: Arc<RwLock<HashSet<String>>>,
    require_auth: bool,
}

impl ApiKeyInterceptor {
    pub fn new(valid_keys: HashSet<String>, require_auth: bool) -> Self {
        Self {
            valid_keys: Arc::new(RwLock::new(valid_keys)),
            require_auth,
        }
    }
    
    pub fn add_key(&self, key: String) {
        self.valid_keys.write().insert(key);
    }
    
    pub fn remove_key(&self, key: &str) {
        self.valid_keys.write().remove(key);
    }
    
    pub fn validate_request<T>(&self, request: &Request<T>) -> Result<(), Status> {
        if !self.require_auth {
            return Ok(());
        }
        
        // Check for API key in metadata
        match request.metadata().get("x-api-key") {
            Some(key_value) => {
                let key = key_value
                    .to_str()
                    .map_err(|_| Status::unauthenticated("Invalid API key format"))?;
                
                let valid_keys = self.valid_keys.read();
                if valid_keys.contains(key) {
                    Ok(())
                } else {
                    Err(Status::unauthenticated("Invalid API key"))
                }
            }
            None => Err(Status::unauthenticated("Missing x-api-key header")),
        }
    }
}

/// Rate limiting interceptor
#[derive(Clone)]
pub struct RateLimitInterceptor {
    limits: Arc<RwLock<std::collections::HashMap<String, RateLimit>>>,
    max_requests_per_minute: u32,
}

struct RateLimit {
    count: u32,
    window_start: std::time::Instant,
}

impl RateLimitInterceptor {
    pub fn new(max_requests_per_minute: u32) -> Self {
        Self {
            limits: Arc::new(RwLock::new(std::collections::HashMap::new())),
            max_requests_per_minute,
        }
    }
    
    pub fn check_rate_limit(&self, client_id: &str) -> Result<(), Status> {
        let mut limits = self.limits.write();
        let now = std::time::Instant::now();
        
        let rate_limit = limits.entry(client_id.to_string()).or_insert(RateLimit {
            count: 0,
            window_start: now,
        });
        
        // Reset window if it's been more than a minute
        if now.duration_since(rate_limit.window_start).as_secs() >= 60 {
            rate_limit.count = 0;
            rate_limit.window_start = now;
        }
        
        if rate_limit.count >= self.max_requests_per_minute {
            return Err(Status::resource_exhausted(
                format!("Rate limit exceeded: {} requests per minute", self.max_requests_per_minute)
            ));
        }
        
        rate_limit.count += 1;
        Ok(())
    }
}

/// Combined auth and rate limit wrapper for gRPC service
pub struct AuthWrapper<S> {
    inner: S,
    api_key_interceptor: ApiKeyInterceptor,
    rate_limiter: Option<RateLimitInterceptor>,
}

impl<S> AuthWrapper<S> {
    pub fn new(
        inner: S,
        api_keys: HashSet<String>,
        require_auth: bool,
        rate_limit: Option<u32>,
    ) -> Self {
        Self {
            inner,
            api_key_interceptor: ApiKeyInterceptor::new(api_keys, require_auth),
            rate_limiter: rate_limit.map(RateLimitInterceptor::new),
        }
    }
    
    pub fn check_auth<T>(&self, request: &Request<T>) -> Result<String, Status> {
        // First check API key
        self.api_key_interceptor.validate_request(request)?;
        
        // Extract client identifier (API key or IP)
        let client_id = request
            .metadata()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("anonymous")
            .to_string();
        
        // Then check rate limit
        if let Some(rate_limiter) = &self.rate_limiter {
            rate_limiter.check_rate_limit(&client_id)?;
        }
        
        Ok(client_id)
    }
}

// Macro to implement auth wrapper for service
#[macro_export]
macro_rules! impl_auth_wrapper {
    ($service:ty) => {
        #[tonic::async_trait]
        impl<S> $service for AuthWrapper<S>
        where
            S: $service,
        {
            type SubscribeOrderbookStream = S::SubscribeOrderbookStream;
            type SubscribeMarkPricesStream = S::SubscribeMarkPricesStream;
            
            async fn subscribe_orderbook(
                &self,
                request: Request<SubscribeRequest>,
            ) -> Result<Response<Self::SubscribeOrderbookStream>, Status> {
                let _client_id = self.check_auth(&request)?;
                self.inner.subscribe_orderbook(request).await
            }
            
            async fn get_orderbook(
                &self,
                request: Request<GetOrderbookRequest>,
            ) -> Result<Response<OrderbookSnapshot>, Status> {
                let _client_id = self.check_auth(&request)?;
                self.inner.get_orderbook(request).await
            }
            
            // ... implement other methods similarly ...
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_api_key_validation() {
        let mut keys = HashSet::new();
        keys.insert("test-key-123".to_string());
        
        let interceptor = ApiKeyInterceptor::new(keys, true);
        
        // Test with valid key
        let mut request = Request::new(());
        request.metadata_mut().insert(
            "x-api-key",
            "test-key-123".parse().unwrap(),
        );
        
        assert!(interceptor.validate_request(&request).is_ok());
        
        // Test with invalid key
        let mut request = Request::new(());
        request.metadata_mut().insert(
            "x-api-key",
            "invalid-key".parse().unwrap(),
        );
        
        assert!(interceptor.validate_request(&request).is_err());
    }
    
    #[test]
    fn test_rate_limiting() {
        let limiter = RateLimitInterceptor::new(5);
        
        // Should allow first 5 requests
        for _ in 0..5 {
            assert!(limiter.check_rate_limit("client1").is_ok());
        }
        
        // 6th request should fail
        assert!(limiter.check_rate_limit("client1").is_err());
        
        // Different client should work
        assert!(limiter.check_rate_limit("client2").is_ok());
    }
}