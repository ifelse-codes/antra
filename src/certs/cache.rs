use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use rustls::server::{ClientHello, ResolvesServerCert};

use crate::certs::ca::CaCert;
use crate::certs::store::CertStore;

/// In-memory certificate cache that resolves certs by SNI.
/// Falls back to disk cache, then generates new certs on demand.
pub struct CertCache {
    certs: RwLock<HashMap<String, Arc<rustls::sign::CertifiedKey>>>,
    store: CertStore,
    ca: CaCert,
}

impl fmt::Debug for CertCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertCache")
            .field(
                "certs_count",
                &self.certs.read().map(|c| c.len()).unwrap_or(0),
            )
            .finish()
    }
}

impl CertCache {
    /// Create a new cache, generating or loading the CA.
    pub fn new() -> Result<Self> {
        let store = CertStore::new()?;
        let ca = store.get_or_create_ca()?;

        tracing::info!("Certificate cache initialized");

        Ok(Self {
            certs: RwLock::new(HashMap::new()),
            store,
            ca,
        })
    }

    /// Resolve or generate a certificate for the given hostname.
    fn resolve_cert(&self, hostname: &str) -> Option<Arc<rustls::sign::CertifiedKey>> {
        // 1. Check memory cache
        if let Some(cert) = self.certs.read().ok()?.get(hostname) {
            return Some(Arc::clone(cert));
        }

        // 2. Check disk cache / generate new
        let leaf = self.store.get_or_create_leaf(hostname, &self.ca).ok()?;

        let certified_key = leaf.to_certified_key().ok()?;

        let key = Arc::new(certified_key);

        // 3. Store in memory cache
        if let Ok(mut cache) = self.certs.write() {
            cache.insert(hostname.to_string(), Arc::clone(&key));
        }

        Some(key)
    }

    /// Get the CA certificate PEM (for trust store installation).
    pub fn ca_cert_pem(&self) -> &str {
        &self.ca.cert_pem
    }
}

impl ResolvesServerCert for CertCache {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<rustls::sign::CertifiedKey>> {
        let sni = hello.server_name()?;
        let hostname = sni.to_string();
        tracing::debug!(%hostname, "SNI resolution request");
        self.resolve_cert(&hostname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cert_cache_resolve() {
        let cache = CertCache::new().unwrap();
        // Just verify it doesn't panic
        let _ = cache.resolve_cert("test.localhost");
    }
}
