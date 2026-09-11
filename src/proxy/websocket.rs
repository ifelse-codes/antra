use anyhow::Result;
use bytes::Bytes;
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};

use crate::routing::types::Route;

/// Dial the upstream for a route, trying 127.0.0.1 first then ::1 when the
/// route points at loopback. Dev servers that bind ::1-only (Vite's default
/// on some machines) refused the old v4-only dial and broke HMR with a bare
/// connect error. Non-loopback routes dial their literal address.
async fn connect_upstream(route: &Route) -> anyhow::Result<tokio::net::TcpStream> {
    if !route.host.is_loopback() {
        let addr = format!("{}:{}", route.host, route.port);
        return tokio::net::TcpStream::connect(&addr).await.map_err(|e| {
            anyhow::anyhow!("Connection to {addr} refused — is your server running? ({e})")
        });
    }
    match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", route.port)).await {
        Ok(s) => Ok(s),
        Err(v4err) => {
            // Note the brackets: bare "::1:port" does not parse as an
            // address; "[::1]:port" does.
            match tokio::net::TcpStream::connect(format!("[::1]:{}", route.port)).await {
                Ok(s) => {
                    tracing::debug!(port = route.port, "Upstream reachable via ::1 only");
                    Ok(s)
                }
                Err(v6err) => Err(anyhow::anyhow!(
                    "Connection to 127.0.0.1:{0} refused ({v4err}) and [::1]:{0} refused ({v6err}) — is your server running?",
                    route.port
                )),
            }
        }
    }
}

/// Check if headers indicate a WebSocket upgrade request.
pub fn is_websocket_upgrade_headers(headers: &hyper::HeaderMap) -> bool {
    let has_upgrade = headers
        .get("upgrade")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("websocket"))
        .unwrap_or(false);

    let has_connection = headers
        .get("connection")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_ascii_lowercase().contains("upgrade"))
        .unwrap_or(false);

    has_upgrade && has_connection
}

/// Check if a request is a WebSocket upgrade request.
pub fn is_websocket_upgrade(req: &Request<Incoming>) -> bool {
    is_websocket_upgrade_headers(req.headers())
}

/// Handle WebSocket upgrade: forward to upstream via raw TCP, tunnel bidirectionally.
pub async fn handle_upgrade(
    req: Request<Incoming>,
    route: &Route,
    hops: u32,
) -> Result<Response<Empty<Bytes>>> {
    if hops >= 5 {
        return Ok(Response::builder()
            .status(StatusCode::LOOP_DETECTED)
            .header("content-type", "text/plain")
            .body(Empty::new())?);
    }

    let original_host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    let upstream_addr = format!("{}:{}", route.host, route.port);
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| "/".to_string());

    // Extract WebSocket headers from client request before consuming
    let ws_key = req
        .headers()
        .get("sec-websocket-key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let ws_version = req
        .headers()
        .get("sec-websocket-version")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("13")
        .to_string();

    let ws_protocol = req
        .headers()
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Auth-relevant client headers the old hand-written request dropped
    // (authenticated sockets + Origin-validating servers broke). Captured
    // before `on(req)` consumes the request. Values are sanitized to a
    // single line to keep the raw request well-formed.
    fn clean_header_value(v: &hyper::HeaderMap, name: &str) -> Option<String> {
        v.get(name)
            .and_then(|h| h.to_str().ok())
            .map(|s| s.replace(['\r', '\n'], "").trim().to_string())
            .filter(|s| !s.is_empty())
    }
    let fwd_headers: Vec<(&str, String)> = [
        "cookie",
        "authorization",
        "origin",
        "sec-websocket-extensions",
        "user-agent",
    ]
    .iter()
    .filter_map(|n| clean_header_value(req.headers(), n).map(|v| (*n, v)))
    .collect();

    // Capture client upgrade mechanism
    let client_upgrade = hyper::upgrade::on(req);

    // Connect to upstream via raw TCP (NOT wrapped in TokioIo yet).
    // Bounded so a dead port fails fast instead of hanging the upgrade.
    // Tries both loopback families so ::1-only servers (Vite default)
    // work for HMR too — see connect_upstream.
    let mut upstream_stream =
        tokio::time::timeout(std::time::Duration::from_secs(10), connect_upstream(route))
            .await
            .map_err(|_| anyhow::anyhow!("Upstream {upstream_addr} connect timed out (10s)"))?
            .map_err(|e| anyhow::anyhow!("Upstream {upstream_addr} connect failed: {e}"))?;

    // Build raw HTTP upgrade request to upstream
    let mut raw_request = format!(
        "GET {path_and_query} HTTP/1.1\r\n\
         host: {upstream_addr}\r\n\
         connection: upgrade\r\n\
         upgrade: websocket\r\n\
         sec-websocket-key: {ws_key}\r\n\
         sec-websocket-version: {ws_version}\r\n"
    );

    if let Some(protocol) = &ws_protocol {
        raw_request.push_str(&format!("sec-websocket-protocol: {protocol}\r\n"));
    }

    // Forward auth-relevant client headers (cookies, Authorization, Origin…)
    for (name, value) in &fwd_headers {
        raw_request.push_str(&format!("{name}: {value}\r\n"));
    }

    // Add forwarded headers
    raw_request.push_str("x-forwarded-for: 127.0.0.1\r\n");
    raw_request.push_str(&format!("x-forwarded-host: {original_host}\r\n"));
    raw_request.push_str("x-forwarded-proto: https\r\n");
    raw_request.push_str(&format!("x-antra-hops: {}\r\n", hops + 1));
    raw_request.push_str("\r\n");

    // Send to upstream
    upstream_stream.write_all(raw_request.as_bytes()).await?;

    // Read 101 response from upstream (bounded — the old loop ran forever
    // on a wedged upstream).
    let mut response_buf = Vec::new();
    let mut temp = [0u8; 4096];
    let read_101 = async {
        loop {
            let n = upstream_stream.read(&mut temp).await?;
            if n == 0 {
                anyhow::bail!("Upstream closed connection during WebSocket upgrade");
            }
            response_buf.extend_from_slice(&temp[..n]);
            if let Ok(s) = std::str::from_utf8(&response_buf) {
                if s.contains("\r\n\r\n") {
                    break;
                }
            }
            if response_buf.len() > 8192 {
                anyhow::bail!("Upstream response too large");
            }
        }
        Ok::<(), anyhow::Error>(())
    };
    tokio::time::timeout(std::time::Duration::from_secs(10), read_101)
        .await
        .map_err(|_| anyhow::anyhow!("Upstream WebSocket handshake timed out (10s)"))??;

    let response_str = std::str::from_utf8(&response_buf)?;
    if !response_str.contains("101") {
        anyhow::bail!(
            "Upstream rejected WebSocket upgrade: {}",
            response_str.lines().next().unwrap_or("")
        );
    }

    tracing::info!("WebSocket tunnel established");

    // Echo upstream 101 headers the old synthetic reply dropped
    // (accept / negotiated protocol / extensions).
    fn upstream_header(response: &str, name: &str) -> Option<String> {
        response.lines().skip(1).find_map(|line| {
            let (k, v) = line.split_once(':')?;
            if k.trim().eq_ignore_ascii_case(name) {
                let v = v.trim().replace(['\r', '\n'], "");
                if v.is_empty() {
                    None
                } else {
                    Some(v)
                }
            } else {
                None
            }
        })
    }
    let mut builder = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("upgrade", "websocket")
        .header("connection", "upgrade");
    if let Some(v) = upstream_header(response_str, "sec-websocket-accept") {
        builder = builder.header("sec-websocket-accept", v);
    }
    if let Some(v) = upstream_header(response_str, "sec-websocket-protocol") {
        builder = builder.header("sec-websocket-protocol", v);
    }
    if let Some(v) = upstream_header(response_str, "sec-websocket-extensions") {
        builder = builder.header("sec-websocket-extensions", v);
    }
    let response = builder.body(Empty::new())?;

    // Spawn tunnel: wrap TcpStream in TokioIo for copy_bidirectional
    tokio::spawn(async move {
        match client_upgrade.await {
            Ok(client_io) => {
                let mut client = TokioIo::new(client_io);
                let mut upstream = upstream_stream;
                if let Err(e) = copy_bidirectional(&mut client, &mut upstream).await {
                    tracing::debug!(error = %e, "WebSocket tunnel closed");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Client WebSocket upgrade failed");
            }
        }
    });

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::types::Protocol;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Instant;

    fn route() -> Route {
        Route {
            domain: "app.localhost".to_string(),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 0, // replaced per test
            pid: None,
            managed: false,
            protocol: Protocol::Http,
            created_at: Instant::now(),
        }
    }

    async fn hold_listener(addr: &str) -> (u16, tokio::task::JoinHandle<()>) {
        try_hold_listener(addr)
            .await
            .unwrap_or_else(|| panic!("test listener failed to bind {addr}"))
    }

    async fn try_hold_listener(addr: &str) -> Option<(u16, tokio::task::JoinHandle<()>)> {
        let listener = tokio::net::TcpListener::bind(addr).await.ok()?;
        let port = listener.local_addr().ok()?.port();
        let handle = tokio::spawn(async move {
            while let Ok((mut s, _)) = listener.accept().await {
                // Hold briefly so the connect handshake completes.
                let _ = tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    tokio::io::AsyncReadExt::read(&mut s, &mut [0u8; 1]),
                )
                .await;
            }
        });
        Some((port, handle))
    }

    #[tokio::test]
    async fn connects_to_ipv4_only_upstream() {
        let (port, handle) = hold_listener("127.0.0.1:0").await;
        let mut r = route();
        r.port = port;
        assert!(connect_upstream(&r).await.is_ok());
        handle.abort();
    }

    #[tokio::test]
    async fn falls_back_to_ipv6_only_upstream() {
        // Regression test: Vite-style ::1-only servers refused the old
        // v4-only dial. Skip where the sandbox has no IPv6 loopback.
        let Some((port, handle)) = try_hold_listener("[::1]:0").await else {
            return;
        };
        let mut r = route();
        r.port = port;
        assert!(connect_upstream(&r).await.is_ok());
        handle.abort();
    }

    #[tokio::test]
    async fn errors_when_nothing_listens() {
        // Ephemeral port with (almost surely) nothing on either family.
        let probe = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let mut r = route();
        r.port = port;
        let err = connect_upstream(&r).await.unwrap_err().to_string();
        assert!(err.contains("is your server running"), "got: {err}");
    }
}
