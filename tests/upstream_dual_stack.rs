//! Guards the assumption behind Antra's dual-stack upstream dialing:
//! hyper's HTTP client must reach a ::1-only server when asked for
//! `http://localhost:<port>/` (i.e. it tries every resolved address, not
//! just the first). `forward_request` relies on this so Vite-style ::1-only
//! dev servers are reachable.

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use std::convert::Infallible;

#[tokio::test]
async fn http_client_reaches_ipv6_only_upstream_via_localhost() {
    // Bind ::1-only. Skip (don't fail) where the sandbox has no IPv6 loopback.
    let listener = match tokio::net::TcpListener::bind("[::1]:0").await {
        Ok(l) => l,
        Err(_) => return,
    };
    let port = listener.local_addr().unwrap().port();

    // Prove 127.0.0.1:<port> really has nothing listening: any success below
    // must have come through ::1 after a v4 refusal.
    assert!(
        tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .is_err(),
        "expected nothing on 127.0.0.1:{port}"
    );

    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                let svc = service_fn(|_req: Request<hyper::body::Incoming>| async {
                    Ok::<_, Infallible>(Response::new(Full::new(Bytes::from("v6-ok"))))
                });
                let _ = hyper::server::conn::http1::Builder::new()
                    .serve_connection(io, svc)
                    .await;
            });
        }
    });

    let client: hyper_util::client::legacy::Client<
        hyper_util::client::legacy::connect::HttpConnector,
        Full<Bytes>,
    > = hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build_http();
    let req = Request::builder()
        .uri(format!("http://localhost:{port}/"))
        .body(Full::new(Bytes::new()))
        .unwrap();
    let resp = tokio::time::timeout(std::time::Duration::from_secs(10), client.request(req))
        .await
        .expect("request timed out")
        .expect("localhost must reach a ::1-only upstream");
    assert_eq!(resp.status(), hyper::StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body[..], b"v6-ok");
}
