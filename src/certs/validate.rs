//! Strict X.509 checks on the certificates Antra mints.
//!
//! Why this exists: OpenSSL and BoringSSL are permissive parsers, so a
//! certificate that is *syntactically* broken can pass every test the project
//! has ever run and still be rejected by a stricter stack. The root CA used
//! to carry `Subject Alternative Name: DNS:Antra Local CA`; a `dNSName` with
//! spaces is not a valid DNS name, and Apple's SecureTransport — the library
//! behind Safari and every macOS system TLS tool — fails at chain-parse with
//! `SSL certificate problem: unsupported or invalid name syntax` even though
//! the certificate is installed and trusted. This module encodes the rules a
//! strict verifier enforces so the failure shows up in `cargo test` instead
//! of in a user's browser.
//!
//! Two rules beyond syntax live here:
//!
//! * **A CA carries no SAN.** A certificate authority is identified by its
//!   subject; names it can vouch for are what its *leafs* are for.
//! * **Validity stays inside Apple's 825-day ceiling** for TLS server
//!   certificates. Apple's requirement (support.apple.com/en-us/103769)
//!   applies to custom roots too — mkcert, the tool Antra positions itself
//!   against, bounds its certificates at "2 years and 3 months, which is
//!   always less than 825 days" for exactly this reason. rcgen's default is
//!   1975→4096, which is outside the window on both ends.

use anyhow::{bail, Result};
use x509_parser::extensions::GeneralName;
use x509_parser::prelude::*;

/// Apple/macOS ceiling for a TLS server certificate's validity window.
pub const MAX_VALIDITY_DAYS: i64 = 825;

/// A leaf with less than this many days left is regenerated rather than
/// served. Comfortably inside the ceiling so a machine that is offline for a
/// few weeks still gets a fresh certificate instead of an expired one.
pub const RENEW_WITHIN_DAYS: i64 = 45;

const SECONDS_PER_DAY: i64 = 86_400;

/// Check a root CA against the rules strict verifiers apply.
pub fn check_ca(der: &[u8]) -> Result<()> {
    let cert = parse(der)?;
    if !cert.is_ca() {
        bail!("CA certificate is missing basicConstraints CA:TRUE");
    }
    if let Some(san) = subject_alt_names(&cert)? {
        // Named explicitly because this is the exact shape that shipped a
        // broken CA to every Apple-stack client: the value looked like a
        // human-readable name, which is exactly what a dNSName must not be.
        bail!(
            "CA certificate must carry no subjectAltName, found: {}",
            describe_names(&san)
        );
    }
    check_validity(&cert, "CA")
}

/// Check a leaf certificate for `hostname`.
///
/// `hostname` is expected to be lowercase; comparison is case-insensitive
/// because DNS is (RFC 4343) and browsers fold before matching.
pub fn check_leaf(der: &[u8], hostname: &str) -> Result<()> {
    let cert = parse(der)?;
    if cert.is_ca() {
        bail!("leaf certificate claims basicConstraints CA:TRUE");
    }

    let names = subject_alt_names(&cert)?;
    let names = names.ok_or_else(|| {
        anyhow::anyhow!("leaf certificate has no subjectAltName — strict verifiers ignore CN")
    })?;
    if names.is_empty() {
        bail!("leaf certificate has an empty subjectAltName");
    }
    for name in &names {
        if let GeneralName::DNSName(dns) = name {
            if !is_valid_dns_name(dns) {
                bail!("leaf subjectAltName {dns:?} is not a syntactically valid DNS name");
            }
            if !dns.eq_ignore_ascii_case(hostname) {
                bail!("leaf subjectAltName {dns:?} does not match hostname {hostname:?}");
            }
        } else {
            bail!("leaf subjectAltName carries a non-DNS name: {name}");
        }
    }

    match cert.extended_key_usage() {
        Ok(Some(eku)) if eku.value.server_auth => {}
        Ok(Some(_)) => bail!("leaf certificate is missing the serverAuth EKU"),
        Ok(None) => bail!("leaf certificate has no extendedKeyUsage (serverAuth required)"),
        Err(e) => bail!("leaf extendedKeyUsage is malformed: {e}"),
    }

    check_validity(&cert, "leaf")
}

/// Whole days until `der` expires. Negative once expired.
pub fn remaining_days(der: &[u8]) -> Result<i64> {
    let cert = parse(der)?;
    Ok((cert.validity().not_after.timestamp() - now_unix()) / SECONDS_PER_DAY)
}

/// True when the certificate expires within [`RENEW_WITHIN_DAYS`] (or has
/// already expired) and should be replaced rather than served.
pub fn needs_renewal(der: &[u8]) -> bool {
    remaining_days(der).is_ok_and(|days| days <= RENEW_WITHIN_DAYS)
}

fn parse(der: &[u8]) -> Result<X509Certificate<'_>> {
    let (_, cert) = X509Certificate::from_der(der)
        .map_err(|e| anyhow::anyhow!("certificate is not valid DER: {e}"))?;
    Ok(cert)
}

fn subject_alt_names<'a>(cert: &X509Certificate<'a>) -> Result<Option<Vec<GeneralName<'a>>>> {
    let san = cert
        .subject_alternative_name()
        .map_err(|e| anyhow::anyhow!("subjectAltName is malformed: {e}"))?;
    Ok(san.map(|san| san.value.general_names.clone()))
}

/// Render SAN entries for an error message: `DNS:a.test` is what a reader
/// needs to see — the raw enum variants read as `DNSName("a.test")`.
fn describe_names(names: &[GeneralName<'_>]) -> String {
    names
        .iter()
        .map(|name| match name {
            GeneralName::DNSName(dns) => format!("DNS:{dns}"),
            GeneralName::IPAddress(_) => "IP:…".to_string(),
            GeneralName::RFC822Name(v) => format!("email:{v}"),
            GeneralName::URI(v) => format!("URI:{v}"),
            other => format!("{other:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn check_validity(cert: &X509Certificate<'_>, what: &str) -> Result<()> {
    let validity = cert.validity();
    let not_before = validity.not_before.timestamp();
    let not_after = validity.not_after.timestamp();
    if not_after <= not_before {
        bail!("{what} certificate expires before it becomes valid");
    }
    let days = (not_after - not_before) / SECONDS_PER_DAY;
    if days > MAX_VALIDITY_DAYS {
        bail!(
            "{what} certificate is valid for {days} days, over the {MAX_VALIDITY_DAYS}-day limit \
             Apple applies to TLS certificates (including from custom roots)"
        );
    }
    if not_before > now_unix() {
        bail!("{what} certificate is not valid yet (notBefore is in the future)");
    }
    Ok(())
}

/// A hostname label check: dot-separated labels of letters, digits and
/// hyphens, no leading/trailing hyphen, no underscores (invalid in a
/// `dNSName`), no wildcard.
fn is_valid_dns_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 253 {
        return false;
    }
    name.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_name_syntax() {
        for good in [
            "myapp.localhost",
            "app.test",
            "a-b.example.internal",
            "x1.y2.z3",
        ] {
            assert!(is_valid_dns_name(good), "{good} should be valid");
        }
        for bad in [
            "Antra Local CA", // the SAN that shipped the broken CA
            "my_app.test",    // underscore is not valid in a dNSName
            "app..test",
            "-app.test",
            "app-.test",
            "",
            "*.app.test",
        ] {
            assert!(!is_valid_dns_name(bad), "{bad} should be rejected");
        }
    }

    #[test]
    fn a_fresh_ca_is_not_flagged_for_renewal() {
        let ca = crate::certs::ca::generate_ca().unwrap();
        assert!(
            !needs_renewal(ca.cert_der.as_ref()),
            "a newly generated CA must not be treated as expiring"
        );
    }
}
