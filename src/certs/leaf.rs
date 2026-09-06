use anyhow::Result;
use rcgen::{CertificateParams, DnType, ExtendedKeyUsagePurpose, KeyUsagePurpose};
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
///
/// The subject CN is the hostname (so issuer != subject and chains build),
/// with SAN + AuthorityKeyIdentifier + serverAuth EKU so OS trust stores and
/// browsers accept it once the Antra CA is trusted.
pub fn generate_leaf_cert(hostname: &str, ca: &CaCert) -> Result<LeafCert> {
    let mut params = CertificateParams::new(vec![hostname.to_string()])?;
    params.distinguished_name.push(DnType::CommonName, hostname);
    params.use_authority_key_identifier_extension = true;
    params.key_usages = vec![
        KeyUsagePurpose::DigitalSignature,
        KeyUsagePurpose::KeyEncipherment,
    ];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
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

    /// The leaf must chain to the CA: subject CN is the hostname (not the
    /// rcgen default), so issuer != subject and OS trust stores can build
    /// the chain once the CA is trusted.
    #[test]
    fn test_leaf_chains_to_ca() {
        let ca = crate::certs::ca::generate_ca().unwrap();
        let leaf = generate_leaf_cert("myapp.localhost", &ca).unwrap();
        let der: &[u8] = leaf.cert_der.as_ref();
        let contains = |needle: &[u8]| der.windows(needle.len()).any(|w| w == needle);
        assert!(
            contains(b"myapp.localhost"),
            "leaf DER must carry the hostname (CN + SAN)"
        );
        assert!(
            contains(b"Antra Local CA"),
            "leaf DER must carry the CA name as issuer"
        );
        assert!(
            !contains(b"rcgen self signed cert"),
            "leaf must not use the generic rcgen subject (breaks chaining)"
        );
    }
}
