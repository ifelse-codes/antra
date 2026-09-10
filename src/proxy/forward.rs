use anyhow::Result;
use hyper::body::Incoming;
use hyper::{Request, Response, Uri};

use crate::proxy::headers;
use crate::routing::types::{Protocol, Route};

/// Forward an incoming request to the upstream server specified by the route.
///
/// Streams the upstream body verbatim (no buffering) so SSE / chunked /
/// infinite streams arrive on time. Headers + status pass through untouched.
pub async fn forward_request(
    req: Request<Incoming>,
    route: &Route,
    hops: u32,
) -> Result<Response<Incoming>> {
    let original_host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let upstream_addr = format!("{}:{}", route.host, route.port);

    // Build upstream URI
    let uri: Uri = format!("http://{upstream_addr}{path_and_query}")
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid upstream URI: {e}"))?;

    // Decompose request to modify parts, then reconstruct
    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    // Upstream is always HTTP/1.1 (browsers may speak H2 to us).
    parts.version = hyper::Version::HTTP_11;

    // Set Host to upstream
    parts.headers.insert(
        hyper::header::HOST,
        upstream_addr
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid host: {e}"))?,
    );

    // Add X-Forwarded-* headers
    // For Phase 4, we assume the proxy received HTTPS from the client
    // (since we terminate TLS). The original protocol is "https".
    headers::set_forwarded_headers_with_parts(&mut parts.headers, &original_host, Protocol::Https);

    // Increment hop count for loop detection
    parts.headers.insert(
        "x-antra-hops",
        (hops + 1)
            .to_string()
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid hop header: {e}"))?,
    );

    let upstream_req = Request::from_parts(parts, body);

    // Send to upstream using hyper-util client. Time out waiting for
    // response HEADERS (time-to-first-byte) so a wedged upstream fails
    // fast instead of hanging the browser forever. The body stream itself
    // is unbounded by design (SSE / infinite streams).
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();

    let upstream_response = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        client.request(upstream_req),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "Upstream {}:{} timed out (no response headers in 30s) — is your server hung?",
            route.host,
            route.port
        )
    })?
    .map_err(|e| {
        anyhow::anyhow!(
            "Connection to {}:{} refused — is your server running? ({e})",
            route.host,
            route.port
        )
    })?;

    // Stream the upstream body verbatim — never collect(). Buffering broke
    // SSE (infinite streams never completed) and spiked memory on large bodies.
    Ok(upstream_response)
}
