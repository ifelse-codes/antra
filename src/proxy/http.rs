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
///
/// Body type is nested Either: Left(Incoming) = live upstream stream,
/// Right(Left(Full)) = static error pages, Right(Right(Empty)) = WS upgrade.
/// Streaming (not buffering) is what makes SSE work through the proxy.
pub async fn handle_request(
    req: Request<Incoming>,
    state: std::sync::Arc<ProxyState>,
) -> Result<Response<Either<Incoming, Either<Full<Bytes>, Empty<Bytes>>>>, anyhow::Error> {
    // Host for routing: HTTP/1.x sends `host`; HTTP/2 sends `:authority`
    // (hyper exposes it via uri host, no `host` header). Check both.
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| req.uri().host().map(|h| h.to_string()))
        .unwrap_or_default();

    let domain = host.split(':').next().unwrap_or(&host).to_ascii_lowercase();

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
                .body(Either::Right(Either::Left(Full::new(Bytes::from(body)))))
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

    // Loop detection: max 5 hops
    const MAX_HOPS: u32 = 5;
    if hops >= MAX_HOPS {
        tracing::warn!(%domain, hops, "Loop detected — max hops exceeded");
        let body = format!(
            "508 Loop Detected\n\n\
             Domain:   {domain}\n\
             Upstream: {}:{}\n\
             Hops:     {hops} (max: {MAX_HOPS})\n\n\
             This usually means:\n\
             1. Your app is proxying to another Antra-managed domain\n\
             2. The Host header is pointing to the wrong upstream\n\n\
             Fix this:\n\
             1. Check your app's proxy configuration\n\
             2. Ensure the Host header matches the upstream server\n\
             3. Use direct localhost URLs instead of Antra domains\n",
            route.host, route.port
        );
        let response = Response::builder()
            .status(508)
            .header("content-type", "text/plain")
            .body(Either::Right(Either::Left(Full::new(Bytes::from(body)))))
            .unwrap();
        return Ok(response);
    }

    // Handle WebSocket upgrade
    if is_ws {
        match websocket::handle_upgrade(req, &route, hops).await {
            Ok(response) => Ok(response.map(|b| Either::Right(Either::Right(b)))),
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
                    .body(Either::Right(Either::Left(Full::new(Bytes::from(body)))))
                    .unwrap();
                Ok(response)
            }
        }
    } else {
        // Regular HTTP forwarding (streamed, not buffered)
        match forward::forward_request(req, &route, hops).await {
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
                    .body(Either::Right(Either::Left(Full::new(Bytes::from(body)))))
                    .unwrap();
                Ok(response)
            }
        }
    }
}
