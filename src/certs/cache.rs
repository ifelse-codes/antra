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
    /// The CA leaves are signed with. Behind a lock, not a plain field: it is
    /// replaced when `antra trust` rotates the CA underneath a running daemon,
    /// and `resolve_cert` reads it on every handshake.
    ca: RwLock<CaCert>,
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
            ca: RwLock::new(ca),
        })
    }

    /// Re-read the CA if it is no longer the one this cache is signing with.
    ///
    /// `antra trust` rotates the CA on disk while the daemon is running. The
    /// trust store then holds the new CA and the daemon still holds the old
    /// one, so every handshake it completes chains to a key the client no
    /// longer trusts. Reloading here — and dropping the memory cache, whose
    /// entries are all signed by the old key — is what makes a rotation
    /// survivable without restarting the daemon.
    fn reload_ca_if_rotated(&self) {
        let Ok(fresh) = self.store.get_or_create_ca() else {
            return;
        };
        let same = match (self.ca.read(), read_ca_fingerprint(&self.store)) {
            (Ok(loaded), Some(on_disk)) => {
                crate::certs::fingerprint(loaded.cert_der.as_ref()) == on_disk
            }
            // No fingerprint to compare against, or the lock is poisoned. Do
            // not churn: the worst case is the pre-fix behaviour.
            _ => true,
        };
        if same {
            return;
        }
        match self.ca.write() {
            Ok(mut ca) => {
                tracing::warn!(
                    "The local CA changed while this daemon was running — reloading it and dropping cached certificates"
                );
                *ca = fresh;
                if let Ok(mut certs) = self.certs.write() {
                    certs.clear();
                }
            }
            Err(e) => tracing::error!(error = %e, "Failed to reload the rotated CA"),
        }
    }

    /// Resolve or generate a certificate for the given hostname.
    fn resolve_cert(&self, hostname: &str) -> Option<Arc<rustls::sign::CertifiedKey>> {
        // 0. If the CA on disk is no longer the one this cache loaded at
        //    startup, reload before doing anything else. A long-lived daemon
        //    otherwise keeps signing with the CA it loaded at boot: after
        //    `antra trust` rotates the CA, the trust store holds the new one
        //    and the daemon still issues leaves from the old key, so *every*
        //    domain fails to verify until the daemon is restarted — and
        //    re-running `antra trust` cannot help, because the rotation is
        //    exactly what already succeeded.
        //
        //    This costs one small file read per SNI resolution that is not
        //    already in memory, which is not a hot path. It is checked before
        //    the memory cache deliberately: a cert cached under the old CA is
        //    just as untrusted as a fresh one, and returning it would hide the
        //    rotation.
        self.reload_ca_if_rotated();

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
        let ca = match self.ca.read() {
            Ok(ca) => ca,
            Err(e) => {
                tracing::error!(%hostname, error = %e, "Failed to read the CA for signing");
                return None;
            }
        };
        let leaf = match self.store.get_or_create_leaf(hostname, &ca) {
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
    pub fn ca_cert_der(&self) -> Vec<u8> {
        self.ca
            .read()
            .map(|ca| ca.cert_der.to_vec())
            .unwrap_or_default()
    }

    /// Fingerprint of the CA this process is serving, reported over IPC so a
    /// CLI can spot a daemon that predates a CA rotation.
    pub fn ca_fingerprint(&self) -> String {
        self.ca
            .read()
            .map(|ca| crate::certs::fingerprint(ca.cert_der.as_ref()))
            .unwrap_or_default()
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

/// Fingerprint of the CA currently on disk, if there is one.
fn read_ca_fingerprint(store: &CertStore) -> Option<String> {
    let pem = std::fs::read_to_string(store.config_dir.join("ca.pem")).ok()?;
    let der = crate::certs::store::pem_to_der(&pem).ok()?;
    Some(crate::certs::fingerprint(&der))
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
