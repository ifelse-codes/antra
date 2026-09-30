use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::certs::cache::CertCache;
use crate::proxy::http::ProxyState;
use crate::routing::registry::RouteRegistry;

/// Bind `port` on both loopback stacks, or on 127.0.0.1 alone when the host
/// has no IPv6 loopback. Any other failure — the port being taken, above all
/// — is an error, so a caller never half-listens. The rule and its reasons
/// live in [`crate::util::port::loopback_binds`], shared with the sync check.
pub async fn bind_loopback(port: u16) -> Result<Vec<TcpListener>> {
    let v4 = TcpListener::bind(("127.0.0.1", port)).await;
    let v6 = TcpListener::bind(("::1", port)).await;
    let listeners = crate::util::port::loopback_binds(v4, v6)?;
    if listeners.len() == 1 {
        tracing::debug!(
            port,
            "No IPv6 loopback on this host; binding 127.0.0.1 only"
        );
    }
    Ok(listeners)
}

/// Bind HTTP→HTTPS redirect listeners on the given port (does not serve).
/// Binds both 127.0.0.1 and ::1 so `localhost` (which prefers ::1 on modern
/// macOS) never falls through to an unrelated IPv6 listener — 127.0.0.1
/// alone on a host with no IPv6 loopback, where no such listener can exist.
pub async fn bind_http_redirect(port: u16) -> Result<Vec<TcpListener>> {
    let listeners = bind_loopback(port).await?;
    for listener in &listeners {
        if let Ok(addr) = listener.local_addr() {
            tracing::info!(%addr, "HTTP→HTTPS redirect listening");
        }
    }
    Ok(listeners)
}

/// Run the HTTP→HTTPS redirect server on an already-bound listener.
/// `https_port` is preserved in the Location header when it isn't the
/// default 443 (otherwise fallback-port users get a dead redirect).
pub fn run_http_redirect(listener: TcpListener, https_port: u16) {
    tokio::spawn(async move {
        loop {
            let (stream, remote_addr) = match listener.accept().await {
                Ok(v) => v,
                Err(e) => {
                    tracing::error!(error = %e, "HTTP redirect accept error");
                    continue;
                }
            };
            tracing::debug!(%remote_addr, "HTTP redirect connection");

            tokio::spawn(async move {
                let io = hyper_util::rt::TokioIo::new(stream);

                let service = hyper::service::service_fn(|req| async move {
                    let host = req
                        .headers()
                        .get("host")
                        .and_then(|h| h.to_str().ok())
                        .unwrap_or("localhost");

                    let host = host.split(':').next().unwrap_or(host);

                    let path = req
                        .uri()
                        .path_and_query()
                        .map(|pq| pq.as_str())
                        .unwrap_or("/");

                    let redirect_url = if https_port == 443 {
                        format!("https://{host}{path}")
                    } else {
                        format!("https://{host}:{https_port}{path}")
                    };

                    let response = hyper::Response::builder()
                        .status(301)
                        .header("location", &redirect_url)
                        .header("content-type", "text/plain")
                        .body(http_body_util::Full::new(bytes::Bytes::from(format!(
                            "Moved to {redirect_url}\n"
                        ))))
                        .unwrap();

                    Ok::<_, anyhow::Error>(response)
                });

                let builder = hyper_util::server::conn::auto::Builder::new(
                    hyper_util::rt::TokioExecutor::new(),
                );
                let conn = builder.serve_connection(io, service);

                if let Err(e) = conn.await {
                    tracing::error!(%remote_addr, error = %e, "Redirect connection error");
                }
            });
        }
    });
}

/// Probe whether a port is bindable on the loopback stacks this host has
/// (quick check, drops the listeners immediately).
pub async fn probe_port(port: u16) -> Result<()> {
    bind_loopback(port).await.map(drop)
}

/// Start the HTTPS proxy server with TLS termination.
/// Listens on both 127.0.0.1 and ::1 (dual-stack loopback), or 127.0.0.1
/// alone on a host with no IPv6 loopback.
pub async fn start_server(
    port: u16,
    registry: Arc<RouteRegistry>,
    cert_cache: Arc<CertCache>,
) -> Result<()> {
    let state = Arc::new(ProxyState::new(registry));

    let provider = rustls::crypto::ring::default_provider();

    let mut tls_config = rustls::ServerConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_cert_resolver(cert_cache);

    // Advertise H2 to browsers (hyper auto-negotiates); upstream stays HTTP/1.1.
    tls_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    // Bind both loopback stacks; fail fast if either is taken so the
    // daemon falls back cleanly instead of half-listening.
    let listeners = bind_loopback(port).await?;
    for listener in &listeners {
        if let Ok(addr) = listener.local_addr() {
            tracing::info!(%addr, "HTTPS proxy listening");
        }
    }

    // One accept loop per listener; all share acceptor + state.
    let mut tasks = Vec::new();
    for listener in listeners {
        let acceptor = acceptor.clone();
        let state = Arc::clone(&state);
        tasks.push(tokio::spawn(async move {
            loop {
                let (stream, remote_addr) = match listener.accept().await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!(error = %e, "HTTPS accept error");
                        continue;
                    }
                };
                tracing::debug!(%remote_addr, "New TLS connection");

                let acceptor = acceptor.clone();
                let state = Arc::clone(&state);

                tokio::spawn(async move {
                    let tls_stream = match acceptor.accept(stream).await {
                        Ok(ts) => ts,
                        Err(e) => {
                            tracing::warn!(%remote_addr, error = %e, "TLS handshake failed");
                            return;
                        }
                    };

                    let io = hyper_util::rt::TokioIo::new(tls_stream);

                    let service = hyper::service::service_fn(move |req| {
                        let state = Arc::clone(&state);
                        async move { crate::proxy::http::handle_request(req, state).await }
                    });

                    let builder = hyper_util::server::conn::auto::Builder::new(
                        hyper_util::rt::TokioExecutor::new(),
                    );
                    let conn = builder.serve_connection_with_upgrades(io, service);

                    if let Err(e) = conn.await {
                        tracing::error!(%remote_addr, error = %e, "TLS connection error");
                    }
                });
            }
        }));
    }

    // Run until all accept loops exit (they don't, unless the task is aborted).
    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

/// Start an HTTP server that redirects all requests to HTTPS.
#[allow(dead_code)]
pub async fn start_http_redirect(port: u16, https_port: u16) -> Result<()> {
    for listener in bind_http_redirect(port).await? {
        run_http_redirect(listener, https_port);
    }
    Ok(())
}
