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
