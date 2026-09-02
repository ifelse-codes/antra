use antra::util::port::find_free_port;
use std::net::TcpListener;

#[test]
fn test_find_free_port_returns_valid_port() {
    let port = find_free_port().unwrap();
    assert!(port > 0);
}

#[test]
fn test_find_free_port_is_bindable() {
    let port = find_free_port().unwrap();
    let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    drop(listener);
}

#[test]
fn test_find_free_port_returns_different_ports() {
    let port1 = find_free_port().unwrap();
    let port2 = find_free_port().unwrap();
    assert!(port1 > 0);
    assert!(port2 > 0);
}
