use tempfile::TempDir;

use antra::certs::ca;
use antra::certs::store::CertStore;

fn temp_cert_store() -> (TempDir, CertStore) {
    let dir = TempDir::new().unwrap();
    let config_dir = dir.path().join("antra");
    let certs_dir = config_dir.join("certs");
    std::fs::create_dir_all(&certs_dir).unwrap();

    let store = CertStore {
        config_dir,
        certs_dir,
    };
    (dir, store)
}

#[test]
fn test_ca_not_exists_initially() {
    let (_dir, store) = temp_cert_store();
    assert!(!store.ca_exists());
}

#[test]
fn test_get_or_create_ca_creates_new() {
    let (_dir, store) = temp_cert_store();
    assert!(!store.ca_exists());

    let ca = store.get_or_create_ca().unwrap();
    assert!(store.ca_exists());
    assert!(!ca.cert_pem.is_empty());
    assert!(!ca.key_pem.is_empty());
}

#[test]
fn test_get_or_create_ca_loads_existing() {
    let (_dir, store) = temp_cert_store();

    let ca1 = store.get_or_create_ca().unwrap();
    let ca2 = store.get_or_create_ca().unwrap();

    assert_eq!(ca1.cert_pem, ca2.cert_pem);
    assert_eq!(ca1.key_pem, ca2.key_pem);
}

#[test]
fn test_save_and_load_ca_roundtrip() {
    let (_dir, store) = temp_cert_store();
    let ca = ca::generate_ca().unwrap();

    store.save_ca(&ca).unwrap();
    assert!(store.ca_exists());

    let loaded = store.load_ca().unwrap();
    assert_eq!(ca.cert_pem, loaded.cert_pem);
    assert_eq!(ca.key_pem, loaded.key_pem);
}

#[test]
fn test_leaf_not_exists_initially() {
    let (_dir, store) = temp_cert_store();
    assert!(!store.leaf_exists("myapp.localhost"));
}

#[test]
fn test_get_or_create_leaf_generates_new() {
    let (_dir, store) = temp_cert_store();
    let ca = store.get_or_create_ca().unwrap();

    let leaf = store.get_or_create_leaf("myapp.localhost", &ca).unwrap();
    assert!(store.leaf_exists("myapp.localhost"));
    assert!(!leaf.cert_pem.is_empty());
    assert!(!leaf.key_pem.is_empty());
}

#[test]
fn test_get_or_create_leaf_loads_existing() {
    let (_dir, store) = temp_cert_store();
    let ca = store.get_or_create_ca().unwrap();

    let leaf1 = store.get_or_create_leaf("test.localhost", &ca).unwrap();
    let leaf2 = store.get_or_create_leaf("test.localhost", &ca).unwrap();

    assert_eq!(leaf1.cert_pem, leaf2.cert_pem);
    assert_eq!(leaf1.key_pem, leaf2.key_pem);
}

#[test]
fn test_leaf_different_hostnames() {
    let (_dir, store) = temp_cert_store();
    let ca = store.get_or_create_ca().unwrap();

    let leaf_a = store.get_or_create_leaf("a.localhost", &ca).unwrap();
    let leaf_b = store.get_or_create_leaf("b.localhost", &ca).unwrap();

    assert_ne!(leaf_a.cert_pem, leaf_b.cert_pem);
    assert!(store.leaf_exists("a.localhost"));
    assert!(store.leaf_exists("b.localhost"));
}

#[test]
fn test_save_and_load_leaf_roundtrip() {
    let (_dir, store) = temp_cert_store();
    let ca = store.get_or_create_ca().unwrap();

    let leaf = antra::certs::leaf::generate_leaf_cert("roundtrip.localhost", &ca).unwrap();
    store.save_leaf("roundtrip.localhost", &leaf).unwrap();

    let loaded = store.load_leaf("roundtrip.localhost").unwrap();
    assert_eq!(leaf.cert_pem, loaded.cert_pem);
    assert_eq!(leaf.key_pem, loaded.key_pem);
}

#[test]
fn test_leaf_key_permissions_on_unix() {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let (_dir, store) = temp_cert_store();
        let ca = store.get_or_create_ca().unwrap();

        store.get_or_create_leaf("perms.localhost", &ca).unwrap();

        let key_path = store.certs_dir.join("perms.localhost-key.pem");
        let metadata = std::fs::metadata(&key_path).unwrap();
        let mode = metadata.permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

/// A CA on disk with no version marker is a pre-rotation CA (every install
/// from Antra ≤ 0.4.0). `get_or_create_ca` must replace it, because the old
/// root carried a `dNSName` that Apple's TLS stack refuses to parse.
#[test]
fn test_ca_without_version_marker_is_rotated() {
    let (_dir, store) = temp_cert_store();
    // Seed a "legacy" CA exactly as an old install would have left it:
    // ca.pem + ca-key.pem, no marker file.
    let legacy = ca::generate_ca().unwrap();
    store.save_ca(&legacy).unwrap();
    assert!(!store.config_dir.join(".ca-version").exists());

    let current = store.get_or_create_ca().unwrap();

    assert_ne!(
        current.cert_pem, legacy.cert_pem,
        "the defective CA must be replaced, not reused"
    );
    assert_eq!(
        store.pending_retired_ca_pem().as_deref(),
        Some(legacy.cert_pem.as_str()),
        "the replaced CA must be kept for exact removal from trust stores"
    );
}

/// A marker written by an older, different version also triggers a rotation —
/// the marker records what is on disk, not what this build wishes were there.
#[test]
fn test_ca_with_stale_marker_version_is_rotated() {
    let (_dir, store) = temp_cert_store();
    let legacy = ca::generate_ca().unwrap();
    store.save_ca(&legacy).unwrap();
    std::fs::write(store.config_dir.join(".ca-version"), "1").unwrap();

    let current = store.get_or_create_ca().unwrap();

    assert_ne!(current.cert_pem, legacy.cert_pem);
    assert!(store.pending_retired_ca_pem().is_some());
}

#[test]
fn test_current_ca_is_loaded_without_rotation() {
    let (_dir, store) = temp_cert_store();
    let first = store.get_or_create_ca().unwrap();
    let second = store.get_or_create_ca().unwrap();

    assert_eq!(first.cert_pem, second.cert_pem);
    assert!(
        store.pending_retired_ca_pem().is_none(),
        "a current CA must not leave a retired CA behind"
    );
}

/// If a rotation was interrupted before the trust store was cleaned up, the
/// retained certificate is the one the user may still have trusted. A second
/// rotation must not overwrite it with a CA that was never installed anywhere.
#[test]
fn test_rotation_never_overwrites_a_pending_retired_ca() {
    let (_dir, store) = temp_cert_store();
    let first_install = ca::generate_ca().unwrap();
    std::fs::write(
        store.config_dir.join("retired-ca.pem"),
        &first_install.cert_pem,
    )
    .unwrap();

    // A CA on disk that nobody ever trusted, plus a stale marker.
    let on_disk = ca::generate_ca().unwrap();
    store.save_ca(&on_disk).unwrap();
    std::fs::write(store.config_dir.join(".ca-version"), "1").unwrap();

    let current = store.get_or_create_ca().unwrap();

    assert_ne!(current.cert_pem, on_disk.cert_pem);
    assert_eq!(
        store.pending_retired_ca_pem().as_deref(),
        Some(first_install.cert_pem.as_str())
    );
}

#[test]
fn test_clear_pending_retired_ca_removes_the_file() {
    let (_dir, store) = temp_cert_store();
    let legacy = ca::generate_ca().unwrap();
    store.save_ca(&legacy).unwrap();
    store.get_or_create_ca().unwrap();
    assert!(store.pending_retired_ca_pem().is_some());

    store.clear_pending_retired_ca();

    assert!(store.pending_retired_ca_pem().is_none());
}

/// The retired root is a different certificate, so the fingerprint the daemon
/// reports over IPC changes with it — that is how `doctor` notices a daemon
/// still serving the old CA.
#[test]
fn test_rotation_changes_the_ca_fingerprint() {
    let (_dir, store) = temp_cert_store();
    let legacy = ca::generate_ca().unwrap();
    store.save_ca(&legacy).unwrap();
    let before = antra::certs::fingerprint(legacy.cert_der.as_ref());

    let current = store.get_or_create_ca().unwrap();

    assert_ne!(antra::certs::fingerprint(current.cert_der.as_ref()), before);
}
