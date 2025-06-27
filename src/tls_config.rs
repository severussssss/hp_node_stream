use anyhow::Result;
use tonic::transport::{Certificate, Identity, ServerTlsConfig};
use std::path::Path;

pub struct TlsConfig {
    pub server_cert: String,
    pub server_key: String,
    pub ca_cert: Option<String>,
}

impl TlsConfig {
    /// Load TLS configuration from files
    pub fn from_files(
        cert_path: impl AsRef<Path>,
        key_path: impl AsRef<Path>,
        ca_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let server_cert = std::fs::read_to_string(cert_path)?;
        let server_key = std::fs::read_to_string(key_path)?;
        
        let ca_cert = match ca_path {
            Some(path) => Some(std::fs::read_to_string(path)?),
            None => None,
        };
        
        Ok(Self {
            server_cert,
            server_key,
            ca_cert,
        })
    }
    
    /// Create server TLS config for tonic
    pub fn server_config(&self) -> Result<ServerTlsConfig> {
        let identity = Identity::from_pem(&self.server_cert, &self.server_key);
        
        let mut config = ServerTlsConfig::new().identity(identity);
        
        // Enable mTLS if CA cert is provided
        if let Some(ca_cert) = &self.ca_cert {
            config = config.client_ca_root(Certificate::from_pem(ca_cert));
        }
        
        Ok(config)
    }
}

/// Generate self-signed certificates for testing
pub fn generate_test_certs() -> Result<()> {
    use std::process::Command;
    
    println!("Generating self-signed certificates for testing...");
    
    // Create certs directory
    std::fs::create_dir_all("certs")?;
    
    // Generate CA key and cert
    Command::new("openssl")
        .args(&[
            "req", "-x509", "-newkey", "rsa:4096",
            "-keyout", "certs/ca-key.pem",
            "-out", "certs/ca-cert.pem",
            "-days", "365",
            "-nodes",
            "-subj", "/C=US/ST=CA/L=SF/O=Test/CN=TestCA"
        ])
        .output()?;
    
    // Generate server key
    Command::new("openssl")
        .args(&[
            "genrsa",
            "-out", "certs/server-key.pem",
            "4096"
        ])
        .output()?;
    
    // Generate server cert request
    Command::new("openssl")
        .args(&[
            "req", "-new",
            "-key", "certs/server-key.pem",
            "-out", "certs/server-req.pem",
            "-subj", "/C=US/ST=CA/L=SF/O=Test/CN=localhost"
        ])
        .output()?;
    
    // Sign server cert
    Command::new("openssl")
        .args(&[
            "x509", "-req",
            "-in", "certs/server-req.pem",
            "-CA", "certs/ca-cert.pem",
            "-CAkey", "certs/ca-key.pem",
            "-CAcreateserial",
            "-out", "certs/server-cert.pem",
            "-days", "365"
        ])
        .output()?;
    
    println!("✓ Certificates generated in ./certs/");
    println!("  - ca-cert.pem    (CA certificate - share with clients)");
    println!("  - server-cert.pem (Server certificate)");
    println!("  - server-key.pem  (Server private key - keep secret!)");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_tls_config_creation() {
        let config = TlsConfig {
            server_cert: "cert".to_string(),
            server_key: "key".to_string(),
            ca_cert: Some("ca".to_string()),
        };
        
        assert!(config.server_config().is_ok());
    }
}