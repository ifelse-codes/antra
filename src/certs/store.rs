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
        // The CA on disk, if there is one yet. Passing it in lets
        // `ensure_leaf_version` also purge leaves signed by a retired CA — a
        // fresh `CertStore` is built on every daemon start and on every
        // `antra trust`, so this is the one place that reliably notices a
        // rotation has happened.
        let existing_ca = std::fs::read_to_string(config_dir.join("ca.pem")).ok();
        ensure_leaf_version(&certs_dir, existing_ca.as_deref())?;
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
        let cert_der = pem_to_der(&cert_pem)?;
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

/// Decode a PEM certificate to DER. `pub` so the SNI cache can fingerprint the
/// on-disk CA without duplicating the base64 handling.
pub fn pem_to_der(pem: &str) -> Result<CertificateDer<'static>> {
    let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    let der = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)?;
    Ok(CertificateDer::from(der))
}

/// Purge cached leaf certs when the leaf format version changes, or when the CA
/// they were signed by is no longer the current one.
///
/// The leaf-format half is the original intent. The CA half is the bug this
/// function was missing, and it is the worse of the two: a leaf signed by a
/// retired CA cannot chain to the CA now in the trust store, so every domain the
/// user had already opened keeps warning *after* a successful `antra trust` —
/// and re-running `antra trust` cannot fix it, because that is the thing that
/// already succeeded.
///
/// The subject name cannot be used to spot this. Every Antra CA is
/// `CN=Antra Local CA`, so a stale leaf's issuer looks identical to the current
/// one; the key is what differs. So the marker records the CA's key
/// fingerprint alongside the format version, and a mismatch purges.
///
/// The CA itself is never touched — it lives outside `certs_dir` — so this costs
/// a re-sign, not a re-trust.
fn ensure_leaf_version(certs_dir: &Path, ca_pem: Option<&str>) -> Result<()> {
    let expected = match ca_pem.and_then(ca_key_fingerprint) {
        Some(fp) => format!("{LEAF_VERSION}:{fp}"),
        // No CA yet (a first run). The format version alone is the right
        // marker; the CA check becomes meaningful from the second run on.
        None => LEAF_VERSION.to_string(),
    };
    let marker = certs_dir.join(".leaf-version");
    let current = std::fs::read_to_string(&marker).unwrap_or_default();
    if current.trim() == expected {
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
    std::fs::write(&marker, expected)?;
    Ok(())
}

/// A short, stable identifier for a CA certificate — not its subject, which is
/// the same string (`CN=Antra Local CA`) for every CA Antra has ever generated.
/// A rotation changes the certificate, so its fingerprint is what distinguishes
/// the current CA from a retired one.
fn ca_key_fingerprint(ca_pem: &str) -> Option<String> {
    let der = pem_to_der(ca_pem).ok()?;
    Some(crate::certs::fingerprint(&der))
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

        ensure_leaf_version(certs, None).unwrap();

        assert!(!certs.join("old.localhost.pem").exists());
        assert_eq!(
            std::fs::read_to_string(certs.join(".leaf-version")).unwrap(),
            LEAF_VERSION
        );

        // Second run is a no-op: fresh leafs survive.
        std::fs::write(certs.join("new.localhost.pem"), "fresh").unwrap();
        ensure_leaf_version(certs, None).unwrap();
        assert!(certs.join("new.localhost.pem").exists());
    }

    /// A leaf signed by a retired CA must not outlive that CA.
    ///
    /// The failure this prevents is invisible from the client side and
    /// unfixable from there: every domain the user had already opened keeps
    /// warning *after* a successful `antra trust`, and re-running `antra trust`
    /// cannot help, because the rotation is the thing that already succeeded.
    /// Confirmed end-to-end on 2026-10-01 — a stale leaf survived a rotation
    /// and a daemon restart and served `Verify return code: 21` until it was
    /// deleted by hand.
    ///
    /// The marker cannot compare CA *subjects*: every Antra CA is
    /// `CN=Antra Local CA`, so the fingerprint is the only thing that tells two
    /// of them apart.
    #[test]
    fn leaves_signed_by_a_retired_ca_are_purged() {
        let dir = tempfile::tempdir().unwrap();
        let certs = dir.path();
        let ca1 = ca::generate_ca().unwrap();
        let ca2 = ca::generate_ca().unwrap();
        assert_ne!(
            crate::certs::fingerprint(ca1.cert_der.as_ref()),
            crate::certs::fingerprint(ca2.cert_der.as_ref()),
            "two generated CAs must differ, or this test proves nothing"
        );

        // The first open has no marker, so it writes one and purges. Only
        // leaves cached *after* that are interesting.
        ensure_leaf_version(certs, Some(&ca1.cert_pem)).unwrap();
        std::fs::write(certs.join("app.localhost.pem"), "signed-by-ca1").unwrap();
        std::fs::write(certs.join("app.localhost-key.pem"), "key").unwrap();
        ensure_leaf_version(certs, Some(&ca1.cert_pem)).unwrap();
        assert!(certs.join("app.localhost.pem").exists());

        // The CA rotates. Opening a store now must drop the stale leaf.
        ensure_leaf_version(certs, Some(&ca2.cert_pem)).unwrap();
        assert!(
            !certs.join("app.localhost.pem").exists(),
            "a leaf signed by the retired CA is still on disk after the rotation"
        );

        // And the marker now records ca2, so this is stable rather than a purge
        // on every open.
        std::fs::write(certs.join("app.localhost.pem"), "signed-by-ca2").unwrap();
        ensure_leaf_version(certs, Some(&ca2.cert_pem)).unwrap();
        assert!(certs.join("app.localhost.pem").exists());
    }
}
