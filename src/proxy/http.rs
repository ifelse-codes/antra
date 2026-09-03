use anyhow::Result;
use bytes::Bytes;
use http_body_util::{Either, Empty, Full};
use hyper::body::Incoming;
use hyper::{Request, Response};

use crate::proxy::{forward, websocket};
use crate::routing::registry::RouteRegistry;

/// Shared state passed to the proxy service.
pub struct ProxyState {
    pub registry: std::sync::Arc<RouteRegistry>,
}

/// Handle an incoming HTTP request: look up route, forward to upstream.
/// For WebSocket upgrades, delegates to the WebSocket handler.
pub async fn handle_request(
    req: Request<Incoming>,
    state: std::sync::Arc<ProxyState>,
) -> Result<Response<Either<Full<Bytes>, Empty<Bytes>>>, anyhow::Error> {
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    let domain = host.split(':').next().unwrap_or(&host).to_string();

    let is_ws = websocket::is_websocket_upgrade(&req);

    tracing::info!(%domain, websocket = is_ws, "Request received");

    // Look up route
    let route = match state.registry.lookup(&domain) {
        Some(route) => route,
        None => {
            tracing::warn!(%domain, "No route found");
            let body = format!(
                "502 Bad Gateway\n\n\
                 Domain: {domain}\n\n\
                 No route is registered for this domain.\n\n\
                 Fix this:\n\
                 1. Register a route: antra run --domain {domain} --port <port> -- <your-command>\n\
                 2. Or add a static route: antra proxy start --route {domain}:<port>\n\
                 3. Check routes: antra list\n"
            );
            let response = Response::builder()
                .status(502)
                .header("content-type", "text/plain")
                .body(Either::Left(Full::new(Bytes::from(body))))
                .unwrap();
            return Ok(response);
        }
    };

    // Get hop count from header
    let hops = req
        .headers()
        .get("x-antra-hops")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(0);

    // Handle WebSocket upgrade
    if is_ws {
        match websocket::handle_upgrade(req, &route, hops).await {
            Ok(response) => Ok(response.map(Either::Right)),
            Err(e) => {
                tracing::error!(%domain, error = %e, "WebSocket upgrade failed");
                let body = format!(
                    "502 Bad Gateway\n\n\
                     Domain:   {domain}\n\
                     Upstream: {}:{}\n\
                     Error:    WebSocket upgrade failed: {e}\n\n\
                     Fix this:\n\
                     1. Is your server running on port {}?\n\
                     2. Does it support WebSocket connections?\n",
                    route.host, route.port, route.port
                );
                let response = Response::builder()
                    .status(502)
                    .header("content-type", "text/plain")
                    .body(Either::Left(Full::new(Bytes::from(body))))
                    .unwrap();
                Ok(response)
            }
        }
    } else {
        // Regular HTTP forwarding
        match forward::forward_request(req, &route).await {
            Ok(response) => Ok(response.map(Either::Left)),
            Err(e) => {
                tracing::error!(%domain, error = %e, "Upstream request failed");
                let body = format!(
                    "503 Service Unavailable\n\n\
                     Domain:   {domain}\n\
                     Upstream: {}:{}\n\
                     Error:    {e}\n\n\
                     Fix this:\n\
                     1. Is your server running on port {}?\n\
                     2. Start it: antra run --domain {domain} --port {} -- <your-command>\n\
                     3. Check routes: antra list\n",
                    route.host, route.port, route.port, route.port
                );
                let response = Response::builder()
                    .status(503)
                    .header("content-type", "text/plain")
                    .body(Either::Left(Full::new(Bytes::from(body))))
                    .unwrap();
                Ok(response)
            }
        }
    }
}
