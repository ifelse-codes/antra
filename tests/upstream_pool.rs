//! The upstream connection pool, proved by counting TCP connections.
//!
//! `forward_request` used to build a `hyper` client per request, so the pool
//! it created was thrown away immediately and every proxied request paid a
//! fresh TCP handshake to the dev server. On an unbundled Vite reload that is
//! hundreds of handshakes for one page — and it was invisible from the CLI,
//! which is why it survived every previous review.
//!
//! The assertion is deliberately about *connections*, not timing: a timing
//! test flakes on a loaded machine, and a passing latency test would not prove
//! the pool is being reused. Counting accepted sockets does.
//!
//! The requests go through the real HTTPS server over TLS, because that is the
//! path a user hits — a test that skipped TLS could pass while the real path
//! still opened a socket per request.

use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use antra::certs::cache::CertCache;
use antra::certs::store::CertStore;
use antra::routing::registry::RouteRegistry;
use antra::routing::types::Route;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsConnector;

/// An upstream that answers every request and counts how many TCP connections
/// it accepted. Keep-alive is left on, so a reusing client is free to reuse.
async fn start_counting_upstream() -> (u16, Arc<AtomicUsize>) {
    let connections = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&connections);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                // One accept can carry many requests once keep-alive works, so
                // this loop runs until the client hangs up.
                loop {
                    let mut buf = vec![0u8; 4096];
                    let Ok(n) = stream.read(&mut buf).await else {
                        return;
                    };
                    if n == 0 {
                        return;
                    }
                    let response = "HTTP/1.1 200 OK\r\ncontent-length: 2\r\n\r\nok";
                    if stream.write_all(response.as_bytes()).await.is_err() {
                        return;
                    }
                }
            });
        }
    });

    (port, connections)
}

/// A rustls client that trusts the proxy's own CA, so the handshake is a real
/// verification rather than an `insecure()` bypass.
fn client_for(ca_der: &[u8]) -> TlsConnector {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(rustls::pki_types::CertificateDer::from(ca_der.to_vec()))
        .expect("CA must be addable as a root");
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    TlsConnector::from(Arc::new(config))
}

#[tokio::test]
async fn sequential_requests_reuse_one_upstream_connection() {
    let (upstream_port, connections) = start_counting_upstream().await;

    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().join("antra");
    let certs_dir = config_dir.join("certs");
    std::fs::create_dir_all(&certs_dir).unwrap();
    // A store over the tempdir: the cache must not touch the real
    // ~/.config/antra just because a test started a server.
    let store = CertStore {
        config_dir: config_dir.clone(),
        certs_dir,
    };
    let cache = Arc::new(CertCache::with_store(store).unwrap());
    let ca_der = cache.ca_cert_der().to_vec();

    let registry = Arc::new(RouteRegistry::new_ephemeral());
    registry
        .register(Route {
            domain: "pool.localhost".to_string(),
            host: IpAddr::from([127, 0, 0, 1]),
            port: upstream_port,
            pid: None,
            managed: false,
            protocol: antra::routing::types::Protocol::Https,
            created_at: std::time::Instant::now(),
        })
        .unwrap();

    let port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        port
    };
    let server = tokio::spawn(antra::proxy::https::start_server(port, registry, cache));
    // Let the listeners come up before the first handshake.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let connector = client_for(&ca_der);
    for i in 0..4 {
        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let server_name = rustls::pki_types::ServerName::try_from("pool.localhost").unwrap();
        let mut tls = connector
            .connect(server_name, tcp)
            .await
            .unwrap_or_else(|e| panic!("request {i}: TLS handshake failed: {e}"));
        tls.write_all(b"GET / HTTP/1.1\r\nhost: pool.localhost\r\nconnection: keep-alive\r\n\r\n")
            .await
            .unwrap();

        let mut buf = vec![0u8; 1024];
        let n = tls.read(&mut buf).await.unwrap();
        let text = String::from_utf8_lossy(&buf[..n]);
        assert!(text.starts_with("HTTP/1.1 200"), "request {i}: {text}");
    }

    // The last accept can land a moment after the final response.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    let accepted = connections.load(Ordering::SeqCst);
    server.abort();

    assert_eq!(
        accepted, 1,
        "4 sequential requests through one daemon must reuse a single upstream \
         connection, but the upstream accepted {accepted}"
    );
}

/// Two different hostnames are two different routes, so they must not share a
/// connection: the pool is keyed per upstream, not per daemon.
#[tokio::test]
async fn requests_to_different_routes_use_separate_connections() {
    let (port_a, conns_a) = start_counting_upstream().await;
    let (port_b, conns_b) = start_counting_upstream().await;

    let temp = tempfile::TempDir::new().unwrap();
    let config_dir = temp.path().join("antra");
    let certs_dir = config_dir.join("certs");
    std::fs::create_dir_all(&certs_dir).unwrap();
    let cache = Arc::new(
        CertCache::with_store(CertStore {
            config_dir,
            certs_dir,
        })
        .unwrap(),
    );
    let ca_der = cache.ca_cert_der().to_vec();

    let registry = Arc::new(RouteRegistry::new_ephemeral());
    for (domain, port) in [("a.localhost", port_a), ("b.localhost", port_b)] {
        registry
            .register(Route {
                domain: domain.to_string(),
                host: IpAddr::from([127, 0, 0, 1]),
                port,
                pid: None,
                managed: false,
                protocol: antra::routing::types::Protocol::Https,
                created_at: std::time::Instant::now(),
            })
            .unwrap();
    }

    let port = {
        let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let p = probe.local_addr().unwrap().port();
        drop(probe);
        p
    };
    let server = tokio::spawn(antra::proxy::https::start_server(port, registry, cache));
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let connector = client_for(&ca_der);
    for (i, host) in ["a.localhost", "b.localhost"].iter().enumerate() {
        let tcp = tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .unwrap();
        let server_name = rustls::pki_types::ServerName::try_from(*host).unwrap();
        let mut tls = connector.connect(server_name, tcp).await.unwrap();
        tls.write_all(format!("GET / HTTP/1.1\r\nhost: {host}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut buf = vec![0u8; 1024];
        let n = tls.read(&mut buf).await.unwrap();
        assert!(
            String::from_utf8_lossy(&buf[..n]).starts_with("HTTP/1.1 200"),
            "request {i} to {host} failed"
        );
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    server.abort();

    assert_eq!(conns_a.load(Ordering::SeqCst), 1, "route a must be reused");
    assert_eq!(conns_b.load(Ordering::SeqCst), 1, "route b must be reused");
}
