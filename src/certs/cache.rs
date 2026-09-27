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
        Self::with_store(CertStore::new()?)
    }

    /// Create a cache over a caller-supplied store.
    ///
    /// `new()` resolves the store from the environment, which in a test means
    /// the developer's real `~/.config/antra` — so an end-to-end test that
    /// starts the HTTPS server needs somewhere else to put the CA.
    pub fn with_store(store: CertStore) -> Result<Self> {
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
        // 1. Check memory cache. A long-lived daemon holds these for the
        //    process lifetime, so a leaf that entered its renewal window
        //    while cached has to be dropped here — otherwise the disk-side
        //    renewal in `CertStore::get_or_create_leaf` would never run.
        match self.certs.read() {
            Ok(certs) => {
                if let Some(cert) = certs.get(hostname) {
                    if !is_expiring(cert) {
                        return Some(Arc::clone(cert));
                    }
                }
            }
            Err(e) => {
                tracing::error!(%hostname, error = %e, "Failed to read cert cache");
            }
        }
        if let Ok(mut cache) = self.certs.write() {
            cache.remove(hostname);
        }

        // 2. Check disk cache / generate new
        let leaf = match self.store.get_or_create_leaf(hostname, &self.ca) {
            Ok(leaf) => leaf,
            Err(e) => {
                tracing::warn!(%hostname, error = %e, "Failed to generate leaf certificate — TLS handshake will fail");
                return None;
            }
        };

        let certified_key = match leaf.to_certified_key() {
            Ok(key) => key,
            Err(e) => {
                tracing::warn!(%hostname, error = %e, "Failed to create CertifiedKey");
                return None;
            }
        };

        let key = Arc::new(certified_key);

        // 3. Store in memory cache
        if let Ok(mut cache) = self.certs.write() {
            cache.insert(hostname.to_string(), Arc::clone(&key));
        }

        Some(key)
    }

    /// The CA this cache signs with, in DER — what a client needs to trust it
    /// without a PEM round-trip.
    ///
    /// Read by the proxy end-to-end test as a trust anchor; nothing in the
    /// binary itself needs it.
    #[allow(dead_code)]
    pub fn ca_cert_der(&self) -> &[u8] {
        &self.ca.cert_der
    }

    /// Fingerprint of the CA this process is serving, reported over IPC so a
    /// CLI can spot a daemon that predates a CA rotation.
    pub fn ca_fingerprint(&self) -> String {
        crate::certs::fingerprint(self.ca.cert_der.as_ref())
    }
}

impl ResolvesServerCert for CertCache {
    fn resolve(&self, hello: ClientHello<'_>) -> Option<Arc<rustls::sign::CertifiedKey>> {
        let sni = match hello.server_name() {
            Some(sni) => sni,
            None => {
                tracing::warn!("TLS handshake failed: no SNI provided by client");
                return None;
            }
        };
        let hostname = sni.to_ascii_lowercase();
        tracing::debug!(%hostname, "SNI resolution request");
        self.resolve_cert(&hostname)
    }
}

/// True when the leaf behind a cached key is inside its renewal window.
fn is_expiring(key: &rustls::sign::CertifiedKey) -> bool {
    key.cert
        .first()
        .is_some_and(|der| crate::certs::validate::needs_renewal(der.as_ref()))
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
