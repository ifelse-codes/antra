//! The regression test for the malformed root CA, run against the TLS stack
//! that actually rejected it.
//!
//! OpenSSL and BoringSSL are permissive parsers, so `cargo test` stayed green
//! as long as it only asked them. Apple's SecureTransport is not: it fails at
//! chain-parse on a `dNSName` that is not a valid DNS name, which is how the
//! root CA carrying `DNS:Antra Local CA` broke Safari and every macOS system
//! TLS tool while passing every previous verification. `/usr/bin/curl` on
//! macOS links SecureTransport, so it is the cheapest faithful stand-in for
//! Safari available without a GUI.
//!
//! Two layers:
//!   1. a hand-rolled TLS server serving an Antra-minted certificate, and
//!   2. the real thing — `antra proxy start` + `antra alias` + curl — so the
//!      CA on disk, the SNI resolver and the forwarder are all in the path.
//!
//! macOS only: on other platforms this file compiles to nothing, because there
//! is no equivalent of the verifier that found the bug.

#![cfg(target_os = "macos")]

mod common;

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use antra::certs::{ca, leaf};
use common::TestHome;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const CURL: &str = "/usr/bin/curl";

fn curl_available() -> bool {
    Path::new(CURL).exists()
}

/// Result of one `curl --cacert` run: (exit code, combined output).
fn curl_https(ca_pem: &Path, host: &str, port: u16) -> (i32, String) {
    let output = Command::new(CURL)
        .args(["--silent", "--show-error", "--max-time", "20", "--cacert"])
        .arg(ca_pem)
        .args([
            "--resolve",
            &format!("{host}:{port}:127.0.0.1"),
            "-o",
            "/dev/null",
            "-w",
            "%{http_code}",
        ])
        .arg(format!("https://{host}:{port}/"))
        .output()
        .expect("failed to run /usr/bin/curl");
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.code().unwrap_or(-1), combined)
}

fn pem_to_der(pem: &str) -> Vec<u8> {
    let body: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, body.trim())
        .expect("PEM body must be base64")
}

/// A port nothing is listening on, chosen by the OS.
fn free_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// One-shot TLS server: accepts a single connection and answers any request
/// with a fixed 200. Enough to prove the handshake and the chain, without
/// pulling a proxy into the first test.
fn serve_one_tls(cert_chain: Vec<Vec<u8>>, key_der: Vec<u8>) -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = std::thread::spawn(move || {
        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(
                cert_chain
                    .into_iter()
                    .map(rustls::pki_types::CertificateDer::from)
                    .collect(),
                rustls::pki_types::PrivateKeyDer::try_from(key_der).unwrap(),
            )
            .expect("server config");

        let acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(config));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            let (socket, _) = match listener.accept().await {
                Ok(pair) => pair,
                Err(_) => return,
            };
            let mut stream = match acceptor.accept(socket).await {
                Ok(stream) => stream,
                Err(_) => return,
            };
            // Read whatever the client sends first so the request completes.
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf).await;
            let body = "ok";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes()).await;
            let _ = stream.flush().await;
        });
    });

    (port, handle)
}

/// Plain HTTP upstream, so the live-daemon test can assert a real 200 through
/// the proxy instead of a 502 (which would also prove the handshake, but says
/// nothing about forwarding).
fn spawn_upstream() -> (u16, std::thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let handle = std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut buf = [0u8; 2048];
            let _ = stream.read(&mut buf);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
            let _ = stream.flush();
        }
    });
    (port, handle)
}

/// An Antra-minted certificate, served directly, must be accepted by Apple's
/// TLS stack when its CA is the only trust anchor.
#[test]
fn securetransport_accepts_antra_certificate() {
    if !curl_available() {
        eprintln!("skipping: /usr/bin/curl not present");
        return;
    }
    let home = TestHome::new();
    let ca = ca::generate_ca().unwrap();
    let leaf = leaf::generate_leaf_cert("app.localhost", &ca).unwrap();

    let ca_path = home.path().join("ca.pem");
    std::fs::write(&ca_path, &ca.cert_pem).unwrap();

    let (port, server) = serve_one_tls(
        vec![leaf.cert_der.to_vec(), ca.cert_der.to_vec()],
        pem_to_der(&leaf.key_pem),
    );

    let (code, output) = curl_https(&ca_path, "app.localhost", port);
    server.join().unwrap();

    assert_eq!(
        code, 0,
        "curl must accept the chain (SecureTransport): {output}"
    );
    assert_eq!(
        output.trim(),
        "200",
        "the served page must come back: {output}"
    );
}

/// The same check through the real proxy: CA on disk, SNI resolution and the
/// forwarder all in the path. This is the command that returned
/// `SSL certificate problem: unsupported or invalid name syntax` while the
/// root CA carried a SAN.
#[test]
fn securetransport_accepts_antra_through_the_daemon() {
    if !curl_available() {
        eprintln!("skipping: /usr/bin/curl not present");
        return;
    }
    let home = TestHome::shared();
    let (upstream_port, _upstream) = spawn_upstream();
    let https_port = free_port();
    let http_port = free_port();

    // A daemon left behind by an interrupted run would answer on the old
    // ports; start from a known state either way.
    let _ = common::run_antra_with_timeout(home, &["proxy", "stop"], Duration::from_secs(20));

    let (start_out, start_err, start_code) = common::run_antra_with_timeout(
        home,
        &[
            "proxy",
            "start",
            "--port",
            &https_port.to_string(),
            "--http-port",
            &http_port.to_string(),
        ],
        Duration::from_secs(20),
    );
    assert_eq!(start_code, 0, "daemon must start: {start_out}{start_err}");

    let (alias_out, alias_err, alias_code) = common::run_antra_with_timeout(
        home,
        &["alias", "app.localhost", &upstream_port.to_string()],
        Duration::from_secs(20),
    );
    assert_eq!(alias_code, 0, "route must register: {alias_out}{alias_err}");

    let ca_path = home.config_dir().join("ca.pem");
    assert!(
        ca_path.exists(),
        "the daemon must have written a CA under the disposable home: {}",
        home.config_dir().display()
    );

    let (code, output) = curl_https(&ca_path, "app.localhost", https_port);

    let _ = common::run_antra_with_timeout(home, &["proxy", "stop"], Duration::from_secs(20));

    assert_eq!(
        code, 0,
        "curl must accept the chain through the proxy (SecureTransport): {output}"
    );
    assert_eq!(
        output.trim(),
        "200",
        "the request must be forwarded, not just handshaken: {output}"
    );
}

/// A client that does not trust the CA must still be refused — "strict about
/// syntax" must not turn into "accepts anything".
#[test]
fn untrusted_client_is_still_rejected() {
    if !curl_available() {
        eprintln!("skipping: /usr/bin/curl not present");
        return;
    }
    let home = TestHome::new();
    let ca = ca::generate_ca().unwrap();
    let leaf = leaf::generate_leaf_cert("app.localhost", &ca).unwrap();

    let other_ca_path = home.path().join("other-ca.pem");
    std::fs::write(&other_ca_path, ca::generate_ca().unwrap().cert_pem).unwrap();

    let (port, server) = serve_one_tls(
        vec![leaf.cert_der.to_vec(), ca.cert_der.to_vec()],
        pem_to_der(&leaf.key_pem),
    );

    let (code, output) = curl_https(&other_ca_path, "app.localhost", port);
    server.join().unwrap();

    assert_ne!(
        code, 0,
        "a chain from an untrusted CA must fail, got: {output}"
    );
}

/// Sensitivity check for the tests above: a CA minted the way Antra ≤ 0.4.0
/// did — `CertificateParams::new(vec!["Antra Local CA"])`, which turns the
/// name into a `dNSName` — must still be *rejected* by Apple's stack, while
/// the current shape is accepted.
///
/// Without this, the passing tests could be passing for reasons unrelated to
/// the SAN, and the regression gate would be decorative.
#[test]
fn securetransport_rejects_the_pre_0_5_ca_shape() {
    if !curl_available() {
        eprintln!("skipping: /usr/bin/curl not present");
        return;
    }
    let home = TestHome::new();

    let mut params = rcgen::CertificateParams::new(vec!["Antra Local CA".to_string()]).unwrap();
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Antra Local CA");
    let key = rcgen::KeyPair::generate().unwrap();
    let legacy_ca = params.self_signed(&key).unwrap();
    let legacy_pem = legacy_ca.pem();

    let (port, server) = serve_one_tls(
        vec![legacy_ca.der().to_vec()],
        pem_to_der(&key.serialize_pem()),
    );

    let ca_path = home.path().join("legacy-ca.pem");
    std::fs::write(&ca_path, &legacy_pem).unwrap();
    let (code, output) = curl_https(&ca_path, "app.localhost", port);
    server.join().unwrap();

    assert_ne!(
        code, 0,
        "the pre-0.5 CA shape must still be rejected, otherwise the tests above \
         prove nothing: {output}"
    );
    assert!(
        output.contains("invalid name syntax") || output.contains("certificate"),
        "expected a certificate rejection, got: {output}"
    );
}
