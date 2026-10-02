//! A CA rotation must not break HTTPS for a daemon that is already running.
//!
//! # Why this test exists
//!
//! On 2026-10-01 a manual check on a Mac found this: a user upgrades to a
//! release that rotates the CA, runs `antra trust`, it **succeeds** — and then
//! every domain they had already opened fails to verify in the browser.
//! Re-running `antra trust` could not help, because the rotation is the thing
//! that had already succeeded. The only cure was deleting
//! `~/.config/antra/certs` and restarting the daemon.
//!
//! Two independent causes, both fixed in #52:
//!
//!   1. `CertCache` loaded the CA once at construction and held it in a plain
//!      field for the process lifetime, so a daemon that stayed up across
//!      `antra trust` kept issuing leaves from the retired key.
//!   2. `CertStore::get_or_create_leaf` returned any on-disk leaf that was not
//!      near expiry, regardless of which CA had signed it.
//!
//! # Why no other suite caught it
//!
//! Every other test in this repository builds a fresh CA and never rotates
//! one, so the whole "upgrading breaks it" class was untested — including the
//! browser check added in #49, which mints its CA seconds before using it. That
//! is the entire reason this file exists: the bug is only reachable by rotating
//! the CA *underneath* something that is already serving.
//!
//! The requests go through the real `proxy::https::start_server` with a real
//! `CertCache` over a real on-disk `CertStore`, because a test that stubbed
//! either could pass while the path a user hits stayed broken. The rotation is
//! performed the way a real one happens — by making the on-disk version marker
//! stale — rather than by poking at internals, so the test exercises the same
//! branch a release would.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use antra::certs::cache::CertCache;
use antra::certs::store::CertStore;
use antra::routing::registry::RouteRegistry;
use antra::routing::types::{Protocol, Route};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsConnector;

fn client_for(ca_der: &[u8]) -> TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(ca_der.to_vec()))
        .expect("CA must be addable as a root");
    TlsConnector::from(Arc::new(
        rustls::ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

/// Build the store the way production does, through `CertStore::at`, so the
/// start-up purge runs. Constructing the struct literally skips it, which is
/// how the first version of this file failed for the wrong reason.
fn store_in(temp: &tempfile::TempDir) -> CertStore {
    CertStore::at(&temp.path().join("antra")).unwrap()
}

/// Force the CA to rotate on the next `get_or_create_ca`, exactly as a build
/// with a newer `CA_VERSION` would. `ca_version_is_current` compares only the
/// marker against the constant, so moving the marker is a faithful stand-in and
/// needs no private field.
fn force_ca_rotation(config_dir: &std::path::Path) {
    std::fs::write(config_dir.join(".ca-version"), "0").unwrap();
}

async fn start_upstream() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let mut buf = vec![0u8; 1024];
                let _ = stream.read(&mut buf).await;
                let body = "ok";
                let _ = stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await;
            });
        }
    });
    port
}

/// Start the real HTTPS server on a free port with one route. Returns the port
/// and the server handle, so a test that restarts the "daemon" can abort the
/// first one — the leaf survives on disk, which is the point of the second
/// test.
async fn start_proxy(cache: Arc<CertCache>, domain: &str, upstream_port: u16) -> (u16, Server) {
    let registry = Arc::new(RouteRegistry::new_ephemeral());
    registry
        .register(Route {
            domain: domain.to_string(),
            host: IpAddr::from([127, 0, 0, 1]),
            port: upstream_port,
            pid: None,
            managed: false,
            protocol: Protocol::Https,
            created_at: Instant::now(),
        })
        .unwrap();

    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = probe.local_addr().unwrap().port();
    drop(probe);

    let server = tokio::spawn(antra::proxy::https::start_server(port, registry, cache));
    // The server binds inside the spawned task, so give it a moment before
    // dialling rather than sleeping after every request.
    tokio::time::sleep(Duration::from_millis(250)).await;
    (port, server)
}

/// The handle `proxy::https::start_server` returns. Its `Result` is not
/// asserted on: a test that cared would hang on the accept loop, and these
/// tests are about what the server *serves*.
type Server = tokio::task::JoinHandle<std::result::Result<(), anyhow::Error>>;

/// GET over TLS, verifying against `ca_der`.
async fn fetch_over_tls(port: u16, domain: &str, ca_der: &[u8]) -> Result<u16, String> {
    let connector = client_for(ca_der);
    let server_name =
        rustls::pki_types::ServerName::try_from(domain.to_string()).map_err(|e| e.to_string())?;
    let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .map_err(|e| format!("connect failed: {e}"))?;
    let mut tls = connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| format!("TLS handshake failed for {domain}: {e}"))?;
    tls.write_all(format!("GET / HTTP/1.1\r\nhost: {domain}\r\n\r\n").as_bytes())
        .await
        .unwrap();
    let mut buf = vec![0u8; 1024];
    let n = tls.read(&mut buf).await.unwrap_or(0);
    let response = String::from_utf8_lossy(&buf[..n]).to_string();
    response
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| format!("no status line: {response:?}"))
}

/// Cause 1: a running daemon must notice the rotation and start serving from
/// the new CA without being restarted.
#[tokio::test]
async fn rotating_the_ca_under_a_running_daemon_keeps_https_working() {
    let temp = tempfile::TempDir::new().unwrap();
    let store = store_in(&temp);
    let config_dir = store.config_dir.clone();
    let cache = Arc::new(CertCache::with_store(store).unwrap());
    let upstream = start_upstream().await;
    let (port, _server) = start_proxy(cache.clone(), "app.localhost", upstream).await;

    // Before: a domain used once, so a leaf is minted and cached on disk.
    let ca_before = cache.ca_cert_der();
    assert_eq!(
        fetch_over_tls(port, "app.localhost", &ca_before[..])
            .await
            .unwrap(),
        200,
        "pre-rotation request failed"
    );

    // Rotate, exactly as a release with a newer CA_VERSION would.
    force_ca_rotation(&config_dir);
    // `at`, not a struct literal: the purge is part of what a rotation does.
    let new_ca = CertStore::at(&config_dir)
        .unwrap()
        .get_or_create_ca()
        .unwrap();
    let ca_after = new_ca.cert_der.to_vec();
    assert_ne!(
        ca_before, ca_after,
        "the CA did not actually rotate, so this test could pass for the wrong reason"
    );

    // Same daemon, same cache object, never restarted. This is what failed.
    let status = fetch_over_tls(port, "app.localhost", &ca_after)
        .await
        .unwrap_or_else(|e| panic!("after a CA rotation HTTPS broke: {e}"));
    assert_eq!(
        status, 200,
        "the proxy must serve certificates from the new CA"
    );

    // Guard the guard: if the retired CA still verified, the assertion above
    // could be satisfied by a cache that never noticed anything.
    assert!(
        fetch_over_tls(port, "app.localhost", &ca_before[..])
            .await
            .is_err(),
        "the retired CA still verified, so this test cannot tell a reload from a no-op"
    );
}

/// Cause 2: a leaf on disk signed by a retired CA must not be served after the
/// daemon restarts. The subject cannot catch this — every Antra CA is
/// `CN=Antra Local CA` — so the check has to be on the key.
#[tokio::test]
async fn a_leaf_from_a_retired_ca_is_not_served_after_a_restart() {
    let temp = tempfile::TempDir::new().unwrap();
    let store = store_in(&temp);
    let config_dir = store.config_dir.clone();
    let certs_dir = config_dir.join("certs");

    // A daemon that mints and caches a leaf under the current CA, then exits.
    {
        let cache = Arc::new(CertCache::with_store(store).unwrap());
        let upstream = start_upstream().await;
        let (port, server) = start_proxy(cache.clone(), "old.localhost", upstream).await;
        let ca_der = cache.ca_cert_der();
        assert_eq!(
            fetch_over_tls(port, "old.localhost", &ca_der[..])
                .await
                .unwrap(),
            200
        );
        server.abort();
    }
    assert!(
        certs_dir.join("old.localhost.pem").exists(),
        "the leaf was not cached on disk, so this test would prove nothing"
    );

    // Rotate, then build a *new* cache over the same directory.
    force_ca_rotation(&config_dir);
    let new_store = CertStore::at(&config_dir).unwrap();
    let new_ca = new_store.get_or_create_ca().unwrap();

    let cache = Arc::new(CertCache::with_store(new_store).unwrap());
    let upstream = start_upstream().await;
    let (port, _server) = start_proxy(cache, "old.localhost", upstream).await;
    let status = fetch_over_tls(port, "old.localhost", &new_ca.cert_der[..])
        .await
        .unwrap_or_else(|e| panic!("a stale leaf was served after the rotation: {e}"));
    assert_eq!(status, 200);
}
