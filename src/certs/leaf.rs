use anyhow::Result;
use rcgen::CertificateParams;
use rustls::pki_types::CertificateDer;

use crate::certs::ca::CaCert;

/// A leaf certificate signed by the CA.
pub struct LeafCert {
    pub cert_der: CertificateDer<'static>,
    pub cert_pem: String,
    pub key_pem: String,
}

impl LeafCert {
    /// Convert to rustls CertifiedKey for use in TLS config.
    pub fn to_certified_key(&self) -> Result<rustls::sign::CertifiedKey> {
        let key_der = rustls_pki_types::PrivateKeyDer::Pkcs8(
            rustls_pki_types::PrivatePkcs8KeyDer::from(pem_to_der(&self.key_pem)?),
        );

        let provider = rustls::crypto::ring::default_provider();

        rustls::sign::CertifiedKey::from_der(vec![self.cert_der.clone()], key_der, &provider)
            .map_err(|e| anyhow::anyhow!("Failed to create CertifiedKey: {e}"))
    }
}

/// Generate a leaf certificate for a specific hostname, signed by the CA.
pub fn generate_leaf_cert(hostname: &str, ca: &CaCert) -> Result<LeafCert> {
    let params = CertificateParams::new(vec![hostname.to_string()])?;
    let key_pair = rcgen::KeyPair::generate()?;

    let issuer = ca.issuer()?;
    let cert = params.signed_by(&key_pair, &issuer)?;

    Ok(LeafCert {
        cert_der: cert.der().clone(),
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    })
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>> {
    let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    Ok(base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &b64,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_leaf_cert() {
        let ca = crate::certs::ca::generate_ca().unwrap();
        let leaf = generate_leaf_cert("myapp.localhost", &ca).unwrap();
        assert!(!leaf.cert_pem.is_empty());
        assert!(!leaf.key_pem.is_empty());
    }
}
