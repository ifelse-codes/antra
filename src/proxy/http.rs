use anyhow::Result;
use bytes::Bytes;
use http_body_util::{Either, Empty, Full};
use hyper::body::Incoming;
use hyper::{Request, Response};

use crate::proxy::{forward, websocket};
use crate::routing::registry::RouteRegistry;
use crate::routing::types::Route;

/// The upstream HTTP client, shared by every request the daemon serves.
pub type UpstreamClient = hyper_util::client::legacy::Client<
    hyper_util::client::legacy::connect::HttpConnector,
    Incoming,
>;

/// Shared state passed to the proxy service.
pub struct ProxyState {
    pub registry: std::sync::Arc<RouteRegistry>,
    /// One client for the daemon's lifetime, not one per request.
    ///
    /// Building a client per request threw away the connection pool, so every
    /// proxied request paid a fresh TCP handshake to the dev server — hundreds
    /// of them on an unbundled Vite reload. The pool dies with this value:
    /// the daemon holds `Arc<ProxyState>` until it exits, so there is no idle
    /// pool outliving the process and nothing extra to clean up on shutdown.
    pub client: UpstreamClient,
}

impl ProxyState {
    pub fn new(registry: std::sync::Arc<RouteRegistry>) -> Self {
        Self {
            registry,
            client: build_upstream_client(),
        }
    }
}

/// Build the pooled upstream client.
///
/// `pool_idle_timeout` is what stops the pool from holding a dead dev server
/// open forever: an idle connection that outlives its backend would hand the
/// next request a reset socket, which surfaces as a 502 the user cannot
/// explain.
fn build_upstream_client() -> UpstreamClient {
    let mut connector = hyper_util::client::legacy::connect::HttpConnector::new();
    connector.set_nodelay(true);
    connector.set_connect_timeout(Some(std::time::Duration::from_secs(10)));
    hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .pool_idle_timeout(std::time::Duration::from_secs(60))
        .pool_max_idle_per_host(8)
        .build(connector)
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
        match forward::forward_request(req, &route, hops, &state.client).await {
            Ok(response) => Ok(response.map(Either::Left)),
            Err(e) => {
                tracing::error!(%domain, error = %e, "Upstream request failed");
                let owner = route_owner(&route).await;
                let body = upstream_down_body(&domain, &route, &e.to_string(), &owner);
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

/// What the daemon can tell about the process behind a route that did not
/// answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteOwner {
    /// A static route (`alias`, `add`): there is no process to ask about.
    Unknown,
    /// The process that registered the route is gone.
    Exited(u32),
    /// Still running. `listening` is every TCP port its process group holds
    /// — empty when the OS would not say.
    Running { pid: u32, listening: Vec<u16> },
}

/// How long one owner lookup answers for. A dev page whose server is down
/// keeps asking — Vite's client pings once a second until it reconnects —
/// and each lookup spawns `lsof` on macOS and `tasklist` on Windows.
const OWNER_CACHE_TTL: std::time::Duration = std::time::Duration::from_secs(2);

type OwnerCache =
    std::sync::Mutex<std::collections::HashMap<u32, (std::time::Instant, RouteOwner)>>;

/// Look up the owner of a managed route. Off the async executor: on Windows
/// the liveness probe spawns `tasklist`, and on macOS the port lookup spawns
/// `lsof`. Only on the error path, never per proxied request, and at most
/// once per `OWNER_CACHE_TTL` per owner.
async fn route_owner(route: &Route) -> RouteOwner {
    static CACHE: std::sync::OnceLock<OwnerCache> = std::sync::OnceLock::new();
    let (true, Some(pid)) = (route.managed, route.pid) else {
        return RouteOwner::Unknown;
    };
    let cache = CACHE.get_or_init(Default::default);
    if let Some((at, owner)) = cache.lock().ok().and_then(|c| c.get(&pid).cloned()) {
        if at.elapsed() < OWNER_CACHE_TTL {
            return owner;
        }
    }
    let owner = tokio::task::spawn_blocking(move || {
        if crate::platform::is_pid_alive(pid) {
            RouteOwner::Running {
                pid,
                listening: crate::util::port::group_listening_ports(pid),
            }
        } else {
            RouteOwner::Exited(pid)
        }
    })
    .await
    .unwrap_or(RouteOwner::Unknown);
    if let Ok(mut c) = cache.lock() {
        // Owners come and go with `antra run`; drop the expired ones so the
        // map stays the size of what is failing right now.
        c.retain(|_, (at, _)| at.elapsed() < OWNER_CACHE_TTL);
        c.insert(pid, (std::time::Instant::now(), owner.clone()));
    }
    owner
}

/// The 503 page for an upstream that did not answer.
///
/// It used to ask "is your server running?" whatever the facts were. For
/// the commonest first-run failure — a server with a hardcoded
/// `listen(3000)` that ignores the `PORT` Antra assigned — the server *was*
/// running, and the page's own fix (`--port 4000`) repeated the wrong port.
/// When the route has an owner, the daemon can check instead of asking.
fn upstream_down_body(domain: &str, route: &Route, error: &str, owner: &RouteOwner) -> String {
    let port = route.port;
    let head = format!(
        "503 Service Unavailable\n\n\
         Domain:   {domain}\n\
         Upstream: {}:{port}\n\
         Error:    {error}\n\n",
        route.host
    );
    let tail = match owner {
        RouteOwner::Unknown => format!(
            "Fix this:\n\
             1. Is your server running on port {port}?\n\
             2. Start it: antra run --domain {domain} --port {port} -- <your-command>\n\
             3. Check routes: antra list\n"
        ),
        RouteOwner::Exited(pid) => format!(
            "The process that registered this route (PID {pid}) has exited.\n\n\
             Fix this:\n\
             1. Start your server again: antra run --domain {domain} -- <your-command>\n\
             2. Or remove routes whose process is gone: antra prune\n"
        ),
        RouteOwner::Running { pid, listening } => {
            let elsewhere: Vec<String> = listening
                .iter()
                .filter(|&&p| p != port)
                .map(u16::to_string)
                .collect();
            match elsewhere.as_slice() {
                [] => format!(
                    "The process behind this route (PID {pid}) is running, but nothing answers on port {port}.\n\n\
                     Fix this:\n\
                     1. If it is still starting, wait a moment and reload.\n\
                     2. If it listens on a fixed port, re-run with that port: antra run --domain {domain} --port <its port> -- <your-command>\n\
                     3. Check routes: antra list\n"
                ),
                [only] => format!(
                    "The process behind this route (PID {pid}) is running, but it is listening on port {only}, not {port}.\n\n\
                     Fix this:\n\
                     1. Re-run with its port: antra run --domain {domain} --port {only} -- <your-command>\n\
                     2. Check routes: antra list\n"
                ),
                many => format!(
                    "The process behind this route (PID {pid}) is running, but it is listening on ports {}, not {port}.\n\n\
                     Fix this:\n\
                     1. Re-run with the one that serves your app: antra run --domain {domain} --port <port> -- <your-command>\n\
                     2. Check routes: antra list\n",
                    many.join(", ")
                ),
            }
        }
    };
    head + &tail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::types::Protocol;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Instant;

    fn route(pid: Option<u32>, managed: bool) -> Route {
        Route {
            domain: "hard.localhost".to_string(),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 4000,
            pid,
            managed,
            protocol: Protocol::Http,
            created_at: Instant::now(),
        }
    }

    fn body(owner: RouteOwner) -> String {
        upstream_down_body(
            "hard.localhost",
            &route(Some(42), true),
            "Could not connect",
            &owner,
        )
    }

    #[test]
    fn a_running_server_on_another_port_is_named_not_doubted() {
        // The A2 case: `listen(3000)` behind a route on the assigned 4000.
        let page = body(RouteOwner::Running {
            pid: 42,
            listening: vec![3000],
        });
        assert!(page.contains("listening on port 3000, not 4000"), "{page}");
        assert!(page.contains("--port 3000"), "{page}");
        assert!(!page.contains("Is your server running"), "{page}");
        // The old page's own fix repeated the wrong port.
        assert!(!page.contains("--port 4000"), "{page}");
    }

    #[test]
    fn a_running_server_with_no_known_port_is_not_called_down() {
        let page = body(RouteOwner::Running {
            pid: 42,
            listening: vec![],
        });
        assert!(page.contains("(PID 42) is running"), "{page}");
        assert!(!page.contains("Is your server running"), "{page}");
        // A listener on the routed port itself is no lead elsewhere.
        let same = body(RouteOwner::Running {
            pid: 42,
            listening: vec![4000],
        });
        assert_eq!(same, page);
    }

    #[test]
    fn several_ports_are_listed_and_none_is_picked() {
        let page = body(RouteOwner::Running {
            pid: 42,
            listening: vec![3000, 4000, 9229],
        });
        assert!(page.contains("ports 3000, 9229, not 4000"), "{page}");
        assert!(page.contains("--port <port>"), "{page}");
    }

    #[test]
    fn an_exited_owner_points_at_prune() {
        let page = body(RouteOwner::Exited(42));
        assert!(page.contains("(PID 42) has exited"), "{page}");
        assert!(page.contains("antra prune"), "{page}");
    }

    #[test]
    fn a_static_route_keeps_the_question() {
        // No owner to check: asking is all the daemon can do.
        let page = upstream_down_body(
            "hard.localhost",
            &route(None, false),
            "Could not connect",
            &RouteOwner::Unknown,
        );
        assert!(
            page.contains("Is your server running on port 4000?"),
            "{page}"
        );
        assert!(page.starts_with("503 Service Unavailable"), "{page}");
    }
}
