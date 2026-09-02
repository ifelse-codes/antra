use std::sync::Arc;

use anyhow::Result;
use tokio::net::TcpListener;

use crate::proxy::http::ProxyState;
use crate::routing::registry::RouteRegistry;

/// Start the HTTP proxy server on the given port.
/// Takes Arc<RouteRegistry> so it can be shared with other components.
pub async fn start_server(port: u16, registry: Arc<RouteRegistry>) -> Result<()> {
    let state = Arc::new(ProxyState { registry });

    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    tracing::info!(%addr, "HTTP proxy listening");

    loop {
        let (stream, remote_addr) = listener.accept().await?;
        tracing::debug!(%remote_addr, "New connection");

        let state = Arc::clone(&state);

        tokio::spawn(async move {
            let io = hyper_util::rt::TokioIo::new(stream);

            let service = hyper::service::service_fn(move |req| {
                let state = Arc::clone(&state);
                async move { crate::proxy::http::handle_request(req, state).await }
            });

            let builder =
                hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new());
            let conn = builder.serve_connection_with_upgrades(io, service);

            if let Err(e) = conn.await {
                tracing::error!(%remote_addr, error = %e, "Connection error");
            }
        });
    }
}
