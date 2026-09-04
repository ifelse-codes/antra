use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn start_echo_server() -> (u16, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let handle = tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(_) => continue,
            };

            tokio::spawn(async move {
                let mut buf = vec![0u8; 4096];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                if n == 0 {
                    return;
                }

                let request = String::from_utf8_lossy(&buf[..n]);
                let is_ws = request.contains("upgrade: websocket")
                    || request.contains("Upgrade: websocket");

                if is_ws {
                    let response = "HTTP/1.1 101 Switching Protocols\r\n\
                                   upgrade: websocket\r\n\
                                   connection: upgrade\r\n\r\n";
                    stream.write_all(response.as_bytes()).await.ok();
                } else {
                    let body = format!("Echo: {}", request.lines().next().unwrap_or(""));
                    let response = format!(
                        "HTTP/1.1 200 OK\r\n\
                         content-length: {}\r\n\
                         connection: close\r\n\r\n\
                         {}",
                        body.len(),
                        body
                    );
                    stream.write_all(response.as_bytes()).await.ok();
                }
            });
        }
    });

    (port, handle)
}

async fn send_raw_request(addr: SocketAddr, request: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    stream.shutdown().await.ok();

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    String::from_utf8_lossy(&response).to_string()
}

#[tokio::test]
async fn test_http_echo_server_direct() {
    let (port, handle) = start_echo_server().await;
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

    let response = send_raw_request(addr, "GET /hello HTTP/1.1\r\nhost: localhost\r\n\r\n").await;

    assert!(response.contains("200 OK"));
    assert!(response.contains("Echo: GET /hello"));

    handle.abort();
}

#[tokio::test]
async fn test_websocket_upgrade_direct() {
    let (port, handle) = start_echo_server().await;
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

    let response = send_raw_request(
        addr,
        "GET /ws HTTP/1.1\r\n\
         host: localhost\r\n\
         upgrade: websocket\r\n\
         connection: Upgrade\r\n\
         sec-websocket-key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
         sec-websocket-version: 13\r\n\r\n",
    )
    .await;

    assert!(response.contains("101 Switching Protocols"));
    assert!(response.contains("upgrade: websocket"));

    handle.abort();
}

#[tokio::test]
async fn test_concurrent_connections() {
    let (port, handle) = start_echo_server().await;
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

    let mut handles = vec![];
    for i in 0..5 {
        handles.push(tokio::spawn(async move {
            let response = send_raw_request(
                addr,
                &format!("GET /test{i} HTTP/1.1\r\nhost: localhost\r\n\r\n"),
            )
            .await;
            assert!(response.contains("200 OK"));
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    handle.abort();
}

#[tokio::test]
async fn test_connection_close_after_response() {
    let (port, handle) = start_echo_server().await;
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);

    let response = send_raw_request(addr, "GET / HTTP/1.1\r\nhost: localhost\r\n\r\n").await;

    assert!(response.contains("connection: close"));

    handle.abort();
}
