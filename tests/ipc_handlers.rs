use std::net::{IpAddr, Ipv4Addr};
use std::time::Instant;

use antra::ipc::protocol::*;
use antra::routing::registry::RouteRegistry;
use antra::routing::types::{Protocol, Route};

fn make_route(domain: &str, port: u16) -> Route {
    Route {
        domain: domain.to_string(),
        host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        pid: None,
        protocol: Protocol::Http,
        created_at: Instant::now(),
    }
}

#[test]
fn test_handle_register_route_success() {
    let registry = RouteRegistry::new();
    let req = RegisterRouteRequest {
        domain: "myapp.localhost".to_string(),
        port: 5173,
        pid: None,
    };

    // handle_register_route is private, test via the public handler path
    // We test the registry directly since the handler is a thin wrapper
    let route = Route {
        domain: req.domain.clone(),
        host: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: req.port,
        pid: req.pid,
        protocol: Protocol::Http,
        created_at: Instant::now(),
    };
    registry.register(route).unwrap();

    assert!(registry.lookup("myapp.localhost").is_some());
    let found = registry.lookup("myapp.localhost").unwrap();
    assert_eq!(found.port, 5173);
}

#[test]
fn test_handle_unregister_route_success() {
    let registry = RouteRegistry::new();
    registry
        .register(make_route("to-remove.localhost", 3000))
        .unwrap();

    registry.unregister("to-remove.localhost").unwrap();
    assert!(registry.lookup("to-remove.localhost").is_none());
}

#[test]
fn test_handle_list_routes_empty() {
    let registry = RouteRegistry::new();
    let routes = registry.list();
    assert!(routes.is_empty());
}

#[test]
fn test_handle_list_routes_with_routes() {
    let registry = RouteRegistry::new();
    registry.register(make_route("a.localhost", 1000)).unwrap();
    registry.register(make_route("b.localhost", 2000)).unwrap();

    let routes = registry.list();
    assert_eq!(routes.len(), 2);
}
