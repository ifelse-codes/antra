use hyper::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Request,
};

use crate::routing::types::Protocol;

/// Set X-Forwarded-* headers using a mutable Request reference.
#[allow(dead_code)]
pub fn set_forwarded_headers<B>(req: &mut Request<B>, original_host: &str, protocol: Protocol) {
    set_forwarded_headers_with_parts(req.headers_mut(), original_host, protocol);
}

/// Set X-Forwarded-* headers on a HeaderMap directly.
pub fn set_forwarded_headers_with_parts(
    headers: &mut HeaderMap,
    original_host: &str,
    protocol: Protocol,
) {
    // X-Forwarded-For: client IP (always localhost for local dev)
    headers.insert(
        HeaderName::from_static("x-forwarded-for"),
        HeaderValue::from_static("127.0.0.1"),
    );

    // X-Forwarded-Proto: original protocol (http or https)
    let proto = match protocol {
        Protocol::Http => "http",
        Protocol::Https => "https",
    };
    headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static(proto),
    );

    // X-Forwarded-Host: original Host header before we overwrote it
    if let Ok(val) = HeaderValue::from_str(original_host) {
        headers.insert(HeaderName::from_static("x-forwarded-host"), val);
    }
}
