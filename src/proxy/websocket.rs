use anyhow::Result;
use bytes::Bytes;
use http_body_util::Empty;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{copy_bidirectional, AsyncReadExt, AsyncWriteExt};

use crate::routing::types::Route;

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

    // Capture client upgrade mechanism
    let client_upgrade = hyper::upgrade::on(req);

    // Connect to upstream via raw TCP (NOT wrapped in TokioIo yet)
    let mut upstream_stream = tokio::net::TcpStream::connect(&upstream_addr).await?;

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

    // Add forwarded headers
    raw_request.push_str("x-forwarded-for: 127.0.0.1\r\n");
    raw_request.push_str(&format!("x-forwarded-host: {original_host}\r\n"));
    raw_request.push_str("x-forwarded-proto: https\r\n");
    raw_request.push_str(&format!("x-antra-hops: {}\r\n", hops + 1));
    raw_request.push_str("\r\n");

    // Send to upstream
    upstream_stream.write_all(raw_request.as_bytes()).await?;

    // Read 101 response from upstream
    let mut response_buf = Vec::new();
    let mut temp = [0u8; 4096];
    loop {
        let n = upstream_stream.read(&mut temp).await?;
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

    let response_str = std::str::from_utf8(&response_buf)?;
    if !response_str.contains("101") {
        anyhow::bail!(
            "Upstream rejected WebSocket upgrade: {}",
            response_str.lines().next().unwrap_or("")
        );
    }

    tracing::info!("WebSocket tunnel established");

    // Return 101 to client
    let response = Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("upgrade", "websocket")
        .header("connection", "upgrade")
        .body(Empty::new())?;

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
