use std::path::Path;

use anyhow::Result;
use rcgen::{CertificateParams, DnType, IsCa, Issuer, KeyPair};
use rustls::pki_types::CertificateDer;

/// Generated CA keypair.
pub struct CaCert {
    pub cert_der: CertificateDer<'static>,
    /// PEM-encoded certificate (for trust store installation)
    pub cert_pem: String,
    /// PEM-encoded private key
    pub key_pem: String,
}

impl CaCert {
    /// Get the signing KeyPair for signing leaf certificates.
    pub fn key_pair(&self) -> Result<KeyPair> {
        Ok(KeyPair::from_pem(&self.key_pem)?)
    }

    /// Create an Issuer from this CA for signing leaf certs.
    pub fn issuer(&self) -> Result<Issuer<'static, KeyPair>> {
        let key_pair = self.key_pair()?;
        Ok(Issuer::from_ca_cert_der(&self.cert_der, key_pair)?)
    }
}

/// Generate a self-signed root CA certificate.
pub fn generate_ca() -> Result<CaCert> {
    let mut params = CertificateParams::new(vec!["Antra Local CA".to_string()])?;
    params.is_ca = IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "Antra Local CA");
    let key_pair = KeyPair::generate()?;
    let cert = params.self_signed(&key_pair)?;

    let cert_der = cert.der().clone();
    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    Ok(CaCert {
        cert_der,
        cert_pem,
        key_pem,
    })
}

/// Load CA from PEM files on disk.
pub fn load_ca_from_pem(cert_path: &Path, key_path: &Path) -> Result<CaCert> {
    let cert_pem = std::fs::read_to_string(cert_path)?;
    let key_pem = std::fs::read_to_string(key_path)?;

    let cert_der = pem_to_cert_der(&cert_pem)?;

    Ok(CaCert {
        cert_der,
        cert_pem,
        key_pem,
    })
}

/// Save CA to PEM files on disk.
///
/// Atomic (temp-file + rename): `get_or_create_ca` runs on first-run paths
/// where parallel test targets or a racing daemon+CLI can generate at once —
/// a plain `write` truncated ca.pem mid-race (`-----END CERTIFICATE-----`
/// cut to `---`), permanently breaking every later load with
/// "Invalid symbol 45" until manual deletion.
pub fn save_ca_to_pem(cert_path: &Path, key_path: &Path, ca: &CaCert) -> Result<()> {
    atomic_write(cert_path, ca.cert_pem.as_bytes(), None)?;

    // Write key with restrictive permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        atomic_write(key_path, ca.key_pem.as_bytes(), Some(0o600))?;
        // Rename preserves the temp file's mode; enforce 0600 regardless.
        std::fs::set_permissions(key_path, std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    {
        atomic_write(key_path, ca.key_pem.as_bytes(), None)?;
    }

    Ok(())
}

/// Write `bytes` to `path` atomically via a temp file in the same directory
/// plus rename.
///
/// Same-directory rename is atomic on POSIX: readers never see a
/// half-written file, even with concurrent generators.
pub(crate) fn atomic_write(
    path: &Path,
    bytes: &[u8],
    #[allow(unused_variables)] mode: Option<u32>,
) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    tmp.write_all(bytes)?;
    #[cfg(unix)]
    if let Some(m) = mode {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(tmp.path(), std::fs::Permissions::from_mode(m))?;
    }
    tmp.persist(path)
        .map_err(|e| anyhow::anyhow!("Failed to persist {}: {e}", path.display()))?;
    // sudo-root daemon writes into the user's config dir: hand the file back
    // so a later unprivileged daemon/CLI can read/overwrite it.
    #[cfg(unix)]
    crate::platform::chown_to_invoking_user(path);
    Ok(())
}

/// Check if CA files exist.
pub fn ca_exists(cert_path: &Path, key_path: &Path) -> bool {
    cert_path.exists() && key_path.exists()
}

fn pem_to_cert_der(pem: &str) -> Result<CertificateDer<'static>> {
    let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    let der = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64)?;
    Ok(CertificateDer::from(der))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_generate_and_save_load_ca() {
        let dir = tempdir().unwrap();
        let cert_path = dir.path().join("ca.pem");
        let key_path = dir.path().join("ca-key.pem");

        let ca = generate_ca().unwrap();
        assert!(!ca_exists(&cert_path, &key_path));

        save_ca_to_pem(&cert_path, &key_path, &ca).unwrap();
        assert!(ca_exists(&cert_path, &key_path));

        let loaded = load_ca_from_pem(&cert_path, &key_path).unwrap();
        assert_eq!(ca.cert_der.as_ref(), loaded.cert_der.as_ref());
        assert_eq!(ca.key_pem, loaded.key_pem);
    }
}
