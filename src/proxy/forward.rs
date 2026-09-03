use anyhow::Result;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::{Request, Response, Uri};

use crate::proxy::headers;
use crate::routing::types::{Protocol, Route};

/// Forward an incoming request to the upstream server specified by the route.
pub async fn forward_request(
    req: Request<Incoming>,
    route: &Route,
) -> Result<Response<Full<Bytes>>> {
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

    let upstream_req = Request::from_parts(parts, body);

    // Send to upstream using hyper-util client
    let client = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
        .build_http();

    let upstream_response = client.request(upstream_req).await.map_err(|e| {
        anyhow::anyhow!(
            "Connection to {}:{} refused — is your server running? ({e})",
            route.host,
            route.port
        )
    })?;

    // Convert response body to Full<Bytes>
    let (resp_parts, body) = upstream_response.into_parts();
    let body_bytes = http_body_util::BodyExt::collect(body)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read upstream body: {e}"))?
        .to_bytes();

    Ok(Response::from_parts(resp_parts, Full::new(body_bytes)))
}
