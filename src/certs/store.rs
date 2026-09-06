use std::path::{Path, PathBuf};

use anyhow::Result;
use rustls::pki_types::CertificateDer;

use crate::certs::ca::{self, CaCert};
use crate::certs::leaf::{self, LeafCert};

/// Leaf certificate format version. Bump to invalidate cached leafs.
/// v2 leafs carry subject CN + AuthorityKeyIdentifier + serverAuth EKU so they
/// chain to the Antra CA; pre-v2 leafs are self-signed-looking and untrusted.
const LEAF_VERSION: &str = "2";

/// Manages CA and leaf certificate storage on disk.
pub struct CertStore {
    pub config_dir: PathBuf,
    pub certs_dir: PathBuf,
}

impl CertStore {
    /// Create a new CertStore rooted at ~/.config/antra/
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
            .join("antra");
        let certs_dir = config_dir.join("certs");
        std::fs::create_dir_all(&certs_dir)?;
        ensure_leaf_version(&certs_dir)?;
        Ok(Self {
            config_dir,
            certs_dir,
        })
    }

    /// Get path to CA certificate PEM.
    fn ca_cert_path(&self) -> PathBuf {
        self.config_dir.join("ca.pem")
    }

    /// Get path to CA private key PEM.
    fn ca_key_path(&self) -> PathBuf {
        self.config_dir.join("ca-key.pem")
    }

    /// Get path to a leaf cert PEM.
    fn leaf_cert_path(&self, hostname: &str) -> PathBuf {
        self.certs_dir.join(format!("{hostname}.pem"))
    }

    /// Get path to a leaf key PEM.
    fn leaf_key_path(&self, hostname: &str) -> PathBuf {
        self.certs_dir.join(format!("{hostname}-key.pem"))
    }

    /// Check if the CA exists on disk.
    pub fn ca_exists(&self) -> bool {
        ca::ca_exists(&self.ca_cert_path(), &self.ca_key_path())
    }

    /// Load CA from disk.
    pub fn load_ca(&self) -> Result<CaCert> {
        ca::load_ca_from_pem(&self.ca_cert_path(), &self.ca_key_path())
    }

    /// Save CA to disk.
    pub fn save_ca(&self, ca: &CaCert) -> Result<()> {
        ca::save_ca_to_pem(&self.ca_cert_path(), &self.ca_key_path(), ca)
    }

    /// Generate or load CA. Creates new one if it doesn't exist.
    pub fn get_or_create_ca(&self) -> Result<CaCert> {
        if self.ca_exists() {
            self.load_ca()
        } else {
            let ca = ca::generate_ca()?;
            self.save_ca(&ca)?;
            tracing::info!("Generated new CA certificate");
            Ok(ca)
        }
    }

    /// Check if a leaf cert exists on disk.
    pub fn leaf_exists(&self, hostname: &str) -> bool {
        self.leaf_cert_path(hostname).exists() && self.leaf_key_path(hostname).exists()
    }

    /// Load a leaf cert from disk.
    pub fn load_leaf(&self, hostname: &str) -> Result<LeafCert> {
        let cert_pem = std::fs::read_to_string(self.leaf_cert_path(hostname))?;
        let key_pem = std::fs::read_to_string(self.leaf_key_path(hostname))?;
        let cert_der = load_pem_cert(&cert_pem)?;
        Ok(LeafCert {
            cert_der,
            cert_pem,
            key_pem,
        })
    }

    /// Save a leaf cert to disk.
    pub fn save_leaf(&self, hostname: &str, leaf: &LeafCert) -> Result<()> {
        std::fs::write(self.leaf_cert_path(hostname), &leaf.cert_pem)?;
        std::fs::write(self.leaf_key_path(hostname), &leaf.key_pem)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                self.leaf_key_path(hostname),
                std::fs::Permissions::from_mode(0o600),
            )?;
        }

        Ok(())
    }

    /// Generate or load a leaf cert for a hostname.
    pub fn get_or_create_leaf(&self, hostname: &str, ca: &CaCert) -> Result<LeafCert> {
        if self.leaf_exists(hostname) {
            return self.load_leaf(hostname);
        }
        let leaf = leaf::generate_leaf_cert(hostname, ca)?;
        self.save_leaf(hostname, &leaf)?;
        tracing::info!(%hostname, "Generated new leaf certificate");
        Ok(leaf)
    }
}

fn load_pem_cert(pem: &str) -> Result<CertificateDer<'static>> {
    let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    let der = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)?;
    Ok(CertificateDer::from(der))
}

/// Purge cached leaf certs when the leaf format version changes.
/// Old leafs can't chain to the CA, so serving them would keep browsers
/// warning even after `antra trust`. The CA itself is kept (re-trust not needed).
fn ensure_leaf_version(certs_dir: &Path) -> Result<()> {
    let marker = certs_dir.join(".leaf-version");
    let current = std::fs::read_to_string(&marker).unwrap_or_default();
    if current.trim() == LEAF_VERSION {
        return Ok(());
    }
    if let Ok(entries) = std::fs::read_dir(certs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "pem") {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
    std::fs::write(&marker, LEAF_VERSION)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ensure_leaf_version_purges_stale_leafs() {
        let dir = tempfile::tempdir().unwrap();
        let certs = dir.path();
        // Simulate a pre-v2 cache: stale leaf + old marker.
        std::fs::write(certs.join("old.localhost.pem"), "stale").unwrap();
        std::fs::write(certs.join(".leaf-version"), "1").unwrap();

        ensure_leaf_version(certs).unwrap();

        assert!(!certs.join("old.localhost.pem").exists());
        assert_eq!(
            std::fs::read_to_string(certs.join(".leaf-version")).unwrap(),
            LEAF_VERSION
        );

        // Second run is a no-op: fresh leafs survive.
        std::fs::write(certs.join("new.localhost.pem"), "fresh").unwrap();
        ensure_leaf_version(certs).unwrap();
        assert!(certs.join("new.localhost.pem").exists());
    }
}
