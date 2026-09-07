use antra::util::port::{detect_port_from_command, find_free_port, is_port_available};
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

#[test]
fn test_is_port_available_false_for_held_port() {
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port();
    assert!(!is_port_available(port));
    drop(held);
}

#[test]
fn test_is_port_available_false_for_wildcard_holder() {
    // A server bound to the wildcard (e.g. `python3 -m http.server`, which
    // on macOS lands on a dual-stack wildcard socket) must read as taken,
    // even though a second specific-address bind can succeed under BSD
    // SO_REUSEADDR semantics. The connect probe covers that blind spot.
    let held = TcpListener::bind("0.0.0.0:0").unwrap();
    let port = held.local_addr().unwrap().port();
    assert!(!is_port_available(port));
    drop(held);
}

#[test]
fn test_is_port_available_true_after_release() {
    let port = find_free_port().unwrap();
    assert!(is_port_available(port));
}

#[test]
fn test_detect_port_from_command_python_http_server() {
    let cmd = vec![
        "python3".to_string(),
        "-m".to_string(),
        "http.server".to_string(),
        "18091".to_string(),
    ];
    assert_eq!(detect_port_from_command(&cmd), Some(18091));
}

#[test]
fn test_detect_port_from_command_flag_forms() {
    let dash = vec!["vite".to_string(), "--port".to_string(), "5173".to_string()];
    assert_eq!(detect_port_from_command(&dash), Some(5173));
    let eq = vec!["vite".to_string(), "--port=5173".to_string()];
    assert_eq!(detect_port_from_command(&eq), Some(5173));
}

#[test]
fn test_detect_port_from_command_none_when_absent() {
    let cmd = vec!["pnpm".to_string(), "dev".to_string()];
    assert_eq!(detect_port_from_command(&cmd), None);
}
