use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use crate::certs::cache::CertCache;
use crate::proxy::http::ProxyState;
use crate::routing::registry::RouteRegistry;

/// Start the HTTPS proxy server with TLS termination.
pub async fn start_server(
    port: u16,
    registry: Arc<RouteRegistry>,
    cert_cache: Arc<CertCache>,
) -> Result<()> {
    let state = Arc::new(ProxyState { registry });

    // Build TLS config using the ring crypto provider
    let provider = rustls::crypto::ring::default_provider();

    let mut tls_config = rustls::ServerConfig::builder_with_provider(provider.into())
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_cert_resolver(cert_cache);

    // Only allow HTTP/1.1 for now (no ALPN)
    tls_config.alpn_protocols = vec![];

    let acceptor = TlsAcceptor::from(Arc::new(tls_config));

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "HTTPS proxy listening");

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        tracing::debug!(%remote_addr, "New TLS connection");

        let acceptor = acceptor.clone();
        let state = Arc::clone(&state);

        tokio::spawn(async move {
            // TLS handshake
            let tls_stream = match acceptor.accept(stream).await {
                Ok(ts) => ts,
                Err(e) => {
                    tracing::error!(%remote_addr, error = %e, "TLS handshake failed");
                    return;
                }
            };

            let io = hyper_util::rt::TokioIo::new(tls_stream);

            let service = hyper::service::service_fn(move |req| {
                let state = Arc::clone(&state);
                async move { crate::proxy::http::handle_request(req, state).await }
            });

            let builder =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
            let conn = builder.serve_connection_with_upgrades(io, service);

            if let Err(e) = conn.await {
                tracing::error!(%remote_addr, error = %e, "TLS connection error");
            }
        });
    }
}

/// Start an HTTP server that redirects all requests to HTTPS.
pub async fn start_http_redirect(port: u16) -> Result<()> {
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "HTTP→HTTPS redirect listening");

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        tracing::debug!(%remote_addr, "HTTP redirect connection");

        tokio::spawn(async move {
            let io = hyper_util::rt::TokioIo::new(stream);

            let service = hyper::service::service_fn(|req| async move {
                let host = req
                    .headers()
                    .get("host")
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("localhost");

                // Strip port if present
                let host = host.split(':').next().unwrap_or(host);

                let path = req
                    .uri()
                    .path_and_query()
                    .map(|pq| pq.as_str())
                    .unwrap_or("/");

                let redirect_url = format!("https://{host}{path}");

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

            let builder =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
            let conn = builder.serve_connection(io, service);

            if let Err(e) = conn.await {
                tracing::error!(%remote_addr, error = %e, "Redirect connection error");
            }
        });
    }
}
