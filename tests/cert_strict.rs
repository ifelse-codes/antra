//! The gate that should have caught the broken root CA.
//!
//! Every assertion here encodes a rule a *strict* X.509 verifier enforces.
//! OpenSSL and BoringSSL — what `openssl verify` and Chrome use — accept
//! certificates that fail these rules, which is why the malformed CA shipped
//! unnoticed: the root carried `DNS:Antra Local CA`, a `dNSName` with spaces
//! that Apple's SecureTransport rejects outright (so did Safari).

use antra::certs::{ca, leaf, store::CertStore, validate};
use tempfile::TempDir;

fn temp_store() -> (TempDir, CertStore) {
    let dir = TempDir::new().unwrap();
    let config_dir = dir.path().join("antra");
    let certs_dir = config_dir.join("certs");
    std::fs::create_dir_all(&certs_dir).unwrap();
    (
        dir,
        CertStore {
            config_dir,
            certs_dir,
        },
    )
}

#[test]
fn ca_carries_no_subject_alt_name() {
    let ca = ca::generate_ca().unwrap();
    validate::check_ca(ca.cert_der.as_ref())
        .unwrap_or_else(|e| panic!("generated CA must pass strict validation: {e:#}"));
}

#[test]
fn ca_validity_stays_inside_apple_limit() {
    let ca = ca::generate_ca().unwrap();
    let remaining = validate::remaining_days(ca.cert_der.as_ref()).unwrap();
    assert!(
        remaining <= validate::MAX_VALIDITY_DAYS,
        "CA must expire within the {}-day limit, got {remaining} days",
        validate::MAX_VALIDITY_DAYS
    );
    assert!(remaining > 0, "a fresh CA must not be expired");
}

#[test]
fn leaf_san_is_the_hostname_and_valid_dns() {
    let ca = ca::generate_ca().unwrap();
    let leaf = leaf::generate_leaf_cert("myapp.localhost", &ca).unwrap();
    validate::check_leaf(leaf.cert_der.as_ref(), "myapp.localhost")
        .unwrap_or_else(|e| panic!("generated leaf must pass strict validation: {e:#}"));
}

#[test]
fn leaf_validity_stays_inside_apple_limit() {
    let ca = ca::generate_ca().unwrap();
    let leaf = leaf::generate_leaf_cert("myapp.localhost", &ca).unwrap();
    let remaining = validate::remaining_days(leaf.cert_der.as_ref()).unwrap();
    assert!(
        remaining <= validate::MAX_VALIDITY_DAYS,
        "leaf must expire within {}-days, got {remaining}",
        validate::MAX_VALIDITY_DAYS
    );
}

#[test]
fn leaf_for_a_custom_domain_also_passes() {
    let ca = ca::generate_ca().unwrap();
    for host in [
        "app.localhost",
        "app.test",
        "app.internal",
        "api.customer.example",
    ] {
        let leaf = leaf::generate_leaf_cert(host, &ca).unwrap();
        validate::check_leaf(leaf.cert_der.as_ref(), host)
            .unwrap_or_else(|e| panic!("leaf for {host} must pass strict validation: {e:#}"));
    }
}

/// A leaf is only useful if it actually chains to the CA, and that is the
/// property the SAN check sits next to: fixing the CA must not break the
/// chain the way the pre-0.2 leafs did.
#[test]
fn leaf_still_chains_to_the_ca() {
    let (_dir, store) = temp_store();
    let ca = store.get_or_create_ca().unwrap();
    let leaf = store.get_or_create_leaf("chain.localhost", &ca).unwrap();
    let needle = b"chain.localhost";
    assert!(
        leaf.cert_der
            .as_ref()
            .windows(needle.len())
            .any(|w| w == needle),
        "leaf must carry the hostname in its SAN"
    );
}

/// A leaf whose hostname does not match its SAN must be rejected — otherwise
/// the SAN check is decorative.
#[test]
fn leaf_for_the_wrong_hostname_is_rejected() {
    let ca = ca::generate_ca().unwrap();
    let leaf = leaf::generate_leaf_cert("myapp.localhost", &ca).unwrap();
    let err = validate::check_leaf(leaf.cert_der.as_ref(), "other.localhost")
        .expect_err("mismatched hostname must be rejected");
    assert!(format!("{err:#}").contains("other.localhost"), "{err:#}");
}
