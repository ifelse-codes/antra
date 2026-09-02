use hyper::header::{HeaderMap, HeaderName, HeaderValue};
use hyper::Request;

use antra::proxy::headers::{set_forwarded_headers, set_forwarded_headers_with_parts};
use antra::routing::types::Protocol;

#[test]
fn test_set_forwarded_headers_http() {
    let mut req = Request::builder().body("test body").unwrap();

    set_forwarded_headers(&mut req, "myapp.localhost", Protocol::Http);

    let headers = req.headers();
    assert_eq!(
        headers.get("x-forwarded-for").unwrap().to_str().unwrap(),
        "127.0.0.1"
    );
    assert_eq!(
        headers.get("x-forwarded-proto").unwrap().to_str().unwrap(),
        "http"
    );
    assert_eq!(
        headers.get("x-forwarded-host").unwrap().to_str().unwrap(),
        "myapp.localhost"
    );
}

#[test]
fn test_set_forwarded_headers_https() {
    let mut req = Request::builder().body("test body").unwrap();

    set_forwarded_headers(&mut req, "secure.test", Protocol::Https);

    let headers = req.headers();
    assert_eq!(
        headers.get("x-forwarded-proto").unwrap().to_str().unwrap(),
        "https"
    );
    assert_eq!(
        headers.get("x-forwarded-host").unwrap().to_str().unwrap(),
        "secure.test"
    );
}

#[test]
fn test_set_forwarded_headers_with_parts() {
    let mut headers = HeaderMap::new();

    set_forwarded_headers_with_parts(&mut headers, "api.localhost:443", Protocol::Https);

    assert_eq!(
        headers.get("x-forwarded-for").unwrap().to_str().unwrap(),
        "127.0.0.1"
    );
    assert_eq!(
        headers.get("x-forwarded-proto").unwrap().to_str().unwrap(),
        "https"
    );
    assert_eq!(
        headers.get("x-forwarded-host").unwrap().to_str().unwrap(),
        "api.localhost:443"
    );
}

#[test]
fn test_set_forwarded_headers_overwrites_existing() {
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-forwarded-for"),
        HeaderValue::from_static("10.0.0.1"),
    );

    set_forwarded_headers_with_parts(&mut headers, "test.localhost", Protocol::Http);

    assert_eq!(
        headers.get("x-forwarded-for").unwrap().to_str().unwrap(),
        "127.0.0.1"
    );
}

#[test]
fn test_set_forwarded_headers_special_chars_in_host() {
    let mut headers = HeaderMap::new();
    set_forwarded_headers_with_parts(&mut headers, "my-app.test:3000", Protocol::Http);

    assert_eq!(
        headers.get("x-forwarded-host").unwrap().to_str().unwrap(),
        "my-app.test:3000"
    );
}

#[test]
fn test_set_forwarded_headers_empty_host() {
    let mut headers = HeaderMap::new();
    set_forwarded_headers_with_parts(&mut headers, "", Protocol::Http);

    let host = headers.get("x-forwarded-host").unwrap().to_str().unwrap();
    assert_eq!(host, "");
}
