use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rustls::pki_types::CertificateDer;

use crate::certs::ca::{self, CaCert};
use crate::certs::leaf::{self, LeafCert};

/// Leaf certificate format version. Bump to invalidate cached leafs.
/// v2 leafs carry subject CN + AuthorityKeyIdentifier + serverAuth EKU so they
/// chain to the Antra CA; pre-v2 leafs are self-signed-looking and untrusted.
/// v3 bounds validity inside Apple's 825-day ceiling and is issued by the
/// SAN-free CA v2 — a v2 leaf is not merely outdated, it can no longer chain
/// to the current root, and its 1975→4096 window is exactly what a strict
/// (Apple) verifier rejects.
const LEAF_VERSION: &str = "3";

/// Root CA format version. Bump to rotate the CA on every existing install.
///
/// v1 (`CertificateParams::new(vec!["Antra Local CA"])`) put the CA's name
/// into `subjectAltName` as a `dNSName`. A `dNSName` must be a syntactically
/// valid DNS name, so Apple's SecureTransport — behind Safari and every
/// macOS system TLS tool — refused the chain at parse time with
/// `SSL certificate problem: unsupported or invalid name syntax` even when
/// the CA was installed and trusted. OpenSSL and BoringSSL tolerate it, which
/// is why the defect survived every previous verification.
///
/// v2 carries no SAN and bounds validity inside Apple's 825-day ceiling.
/// Rotating the root is the only way to fix installs that already minted a
/// v1 CA: existing leafs cannot chain to the new root, which is why
/// `LEAF_VERSION` moves in the same release.
const CA_VERSION: &str = "2";

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

    pub fn read_existing_ca_pem(config_dir: &Path) -> Result<Option<String>> {
        let path = config_dir.join("ca.pem");
        match std::fs::read_to_string(&path) {
            Ok(pem) => Ok(Some(pem)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e).with_context(|| format!("Failed to read {}", path.display())),
        }
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

    /// Generate or load the CA, rotating it when the on-disk one predates
    /// [`CA_VERSION`].
    ///
    /// This is the single funnel every CA consumer goes through — the daemon
    /// (`CertCache::new`) and the CLI (`trust::*`) — so a rotation always
    /// happens before anything binds a TLS listener, and no process can end
    /// up serving a retired root.
    pub fn get_or_create_ca(&self) -> Result<CaCert> {
        let had_ca = self.ca_exists();
        if had_ca && self.ca_version_is_current() {
            return self.load_ca();
        }

        if had_ca {
            self.retain_retired_ca()?;
        }

        let ca = ca::generate_ca()?;
        self.save_ca(&ca)?;
        // Marker last: a crash between the atomic CA write and this line
        // simply rotates again on the next run, which is recoverable. The
        // reverse order would stamp a retired CA as current.
        self.write_ca_version()?;
        if had_ca {
            tracing::warn!(
                "Rotated the local CA to version {CA_VERSION} — the previous one is no longer \
                 trusted, so HTTPS needs re-trusting"
            );
        } else {
            tracing::info!("Generated new CA certificate (version {CA_VERSION})");
        }
        Ok(ca)
    }

    fn ca_version_path(&self) -> PathBuf {
        self.config_dir.join(".ca-version")
    }

    fn retired_ca_path(&self) -> PathBuf {
        self.config_dir.join("retired-ca.pem")
    }

    /// The retired CA, if one is waiting to be dropped from trust stores.
    ///
    /// Set by a rotation and cleared by `trust` once the new CA is trusted.
    /// It survives crashes so an interrupted rotation still converges.
    pub fn pending_retired_ca_pem(&self) -> Option<String> {
        let pem = std::fs::read_to_string(self.retired_ca_path()).ok()?;
        pem.contains("-----BEGIN CERTIFICATE-----").then_some(pem)
    }

    /// Forget the retired CA once it has been removed from the trust stores.
    pub fn clear_pending_retired_ca(&self) {
        if let Err(e) = std::fs::remove_file(self.retired_ca_path()) {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(error = %e, "Failed to clear the retired CA file");
            }
        }
    }

    fn ca_version_is_current(&self) -> bool {
        std::fs::read_to_string(self.ca_version_path())
            .map(|v| v.trim() == CA_VERSION)
            .unwrap_or(false)
    }

    fn write_ca_version(&self) -> Result<()> {
        ca::atomic_write(&self.ca_version_path(), CA_VERSION.as_bytes(), None)
    }

    /// Copy the CA being replaced aside so `trust` can remove exactly those
    /// bytes from the OS trust stores afterwards.
    ///
    /// Never overwrites an existing file: if a previous rotation never got
    /// cleaned up, that file is the certificate the user may still have
    /// trusted, and the one on disk was never installed anywhere.
    fn retain_retired_ca(&self) -> Result<()> {
        if self.pending_retired_ca_pem().is_some() {
            return Ok(());
        }
        let pem = std::fs::read_to_string(self.ca_cert_path())
            .with_context(|| format!("Failed to read {}", self.ca_cert_path().display()))?;
        ca::atomic_write(&self.retired_ca_path(), pem.as_bytes(), Some(0o600))?;
        tracing::warn!("Retiring the existing CA certificate — it will be replaced");
        Ok(())
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

    /// Save a leaf cert to disk (atomic — see save_ca_to_pem).
    pub fn save_leaf(&self, hostname: &str, leaf: &LeafCert) -> Result<()> {
        crate::certs::ca::atomic_write(
            &self.leaf_cert_path(hostname),
            leaf.cert_pem.as_bytes(),
            None,
        )?;
        #[cfg(unix)]
        {
            crate::certs::ca::atomic_write(
                &self.leaf_key_path(hostname),
                leaf.key_pem.as_bytes(),
                Some(0o600),
            )?;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(
                self.leaf_key_path(hostname),
                std::fs::Permissions::from_mode(0o600),
            )?;
            Ok(())
        }
        #[cfg(not(unix))]
        {
            crate::certs::ca::atomic_write(
                &self.leaf_key_path(hostname),
                leaf.key_pem.as_bytes(),
                None,
            )?;
            Ok(())
        }
    }

    /// Generate or load a leaf cert for a hostname.
    ///
    /// A cached leaf inside its renewal window is replaced rather than
    /// served. Validity is now bounded (Apple's 825-day ceiling for TLS
    /// server certs, `certs::VALIDITY_DAYS`), so unlike the old
    /// never-expiring leafs these do eventually need replacing — and a
    /// machine that was off for a few weeks should not come back to an
    /// expired certificate.
    pub fn get_or_create_leaf(&self, hostname: &str, ca: &CaCert) -> Result<LeafCert> {
        if self.leaf_exists(hostname) {
            let existing = self.load_leaf(hostname)?;
            if !crate::certs::validate::needs_renewal(existing.cert_der.as_ref()) {
                return Ok(existing);
            }
            tracing::info!(
                %hostname,
                days_left = crate::certs::validate::remaining_days(existing.cert_der.as_ref())
                    .unwrap_or_default(),
                "Leaf certificate is inside its renewal window — regenerating"
            );
            // Keep the still-valid old leaf as a fallback: if re-signing
            // fails there is no reason to fail the handshake.
            return match self.write_new_leaf(hostname, ca) {
                Ok(leaf) => Ok(leaf),
                Err(e) => {
                    tracing::warn!(%hostname, error = %e, "Leaf renewal failed — serving the existing certificate");
                    Ok(existing)
                }
            };
        }
        self.write_new_leaf(hostname, ca)
    }

    fn write_new_leaf(&self, hostname: &str, ca: &CaCert) -> Result<LeafCert> {
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
    fn test_read_existing_ca_pem_does_not_create_config_dir() {
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("antra");

        assert_eq!(CertStore::read_existing_ca_pem(&config_dir).unwrap(), None);
        assert!(!config_dir.exists());

        std::fs::create_dir(&config_dir).unwrap();
        std::fs::write(config_dir.join("ca.pem"), "existing").unwrap();
        assert_eq!(
            CertStore::read_existing_ca_pem(&config_dir).unwrap(),
            Some("existing".to_string())
        );
    }

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
