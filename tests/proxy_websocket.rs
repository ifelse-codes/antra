use hyper::header::HeaderMap;

use antra::proxy::websocket::is_websocket_upgrade_headers;

fn make_headers(headers: &[(&str, &str)]) -> HeaderMap {
    let mut map = HeaderMap::new();
    for (k, v) in headers {
        map.insert(
            k.parse::<hyper::header::HeaderName>().unwrap(),
            v.parse::<hyper::header::HeaderValue>().unwrap(),
        );
    }
    map
}

#[test]
fn test_is_websocket_upgrade_valid() {
    let headers = make_headers(&[
        ("upgrade", "websocket"),
        ("connection", "Upgrade"),
        ("sec-websocket-key", "dGhlIHNhbXBsZSBub25jZQ=="),
        ("sec-websocket-version", "13"),
    ]);
    assert!(is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_missing_upgrade_header() {
    let headers = make_headers(&[("connection", "Upgrade"), ("sec-websocket-key", "abc")]);
    assert!(!is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_missing_connection_header() {
    let headers = make_headers(&[("upgrade", "websocket"), ("sec-websocket-key", "abc")]);
    assert!(!is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_wrong_upgrade_value() {
    let headers = make_headers(&[("upgrade", "h2c"), ("connection", "Upgrade")]);
    assert!(!is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_case_insensitive() {
    let headers = make_headers(&[("upgrade", "WebSocket"), ("connection", "upgrade")]);
    assert!(is_websocket_upgrade_headers(&headers));

    let headers = make_headers(&[("upgrade", "WEBSOCKET"), ("connection", "Upgrade")]);
    assert!(is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_connection_contains_upgrade() {
    let headers = make_headers(&[
        ("upgrade", "websocket"),
        ("connection", "keep-alive, Upgrade"),
    ]);
    assert!(is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_no_headers() {
    let headers = HeaderMap::new();
    assert!(!is_websocket_upgrade_headers(&headers));
}

#[test]
fn test_is_websocket_upgrade_regular_http() {
    let headers = make_headers(&[("host", "myapp.localhost"), ("accept", "*/*")]);
    assert!(!is_websocket_upgrade_headers(&headers));
}
