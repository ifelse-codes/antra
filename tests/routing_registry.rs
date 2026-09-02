use std::net::{IpAddr, Ipv4Addr};
use std::time::Instant;

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
fn test_register_and_lookup() {
    let registry = RouteRegistry::new();
    let route = make_route("myapp.localhost", 5173);

    registry.register(route).unwrap();
    let found = registry.lookup("myapp.localhost").unwrap();

    assert_eq!(found.domain, "myapp.localhost");
    assert_eq!(found.port, 5173);
}

#[test]
fn test_lookup_nonexistent_returns_none() {
    let registry = RouteRegistry::new();
    assert!(registry.lookup("nonexistent.localhost").is_none());
}

#[test]
fn test_unregister_existing() {
    let registry = RouteRegistry::new();
    registry.register(make_route("a.localhost", 3000)).unwrap();

    registry.unregister("a.localhost").unwrap();
    assert!(registry.lookup("a.localhost").is_none());
}

#[test]
fn test_unregister_nonexistent_is_ok() {
    let registry = RouteRegistry::new();
    registry.unregister("nonexistent.localhost").unwrap();
}

#[test]
fn test_overwrite_domain() {
    let registry = RouteRegistry::new();
    registry
        .register(make_route("app.localhost", 3000))
        .unwrap();
    registry
        .register(make_route("app.localhost", 4000))
        .unwrap();

    let found = registry.lookup("app.localhost").unwrap();
    assert_eq!(found.port, 4000);
}

#[test]
fn test_list_empty() {
    let registry = RouteRegistry::new();
    assert!(registry.list().is_empty());
}

#[test]
fn test_list_multiple_routes() {
    let registry = RouteRegistry::new();
    registry.register(make_route("a.localhost", 1000)).unwrap();
    registry.register(make_route("b.localhost", 2000)).unwrap();
    registry.register(make_route("c.localhost", 3000)).unwrap();

    let routes = registry.list();
    assert_eq!(routes.len(), 3);

    let domains: Vec<String> = routes.iter().map(|r| r.domain.clone()).collect();
    assert!(domains.contains(&"a.localhost".to_string()));
    assert!(domains.contains(&"b.localhost".to_string()));
    assert!(domains.contains(&"c.localhost".to_string()));
}

#[test]
fn test_default_impl() {
    let registry = RouteRegistry::default();
    assert!(registry.list().is_empty());
}

#[test]
fn test_register_with_pid() {
    let registry = RouteRegistry::new();
    let mut route = make_route("pid-test.localhost", 8080);
    route.pid = Some(12345);

    registry.register(route).unwrap();
    let found = registry.lookup("pid-test.localhost").unwrap();
    assert_eq!(found.pid, Some(12345));
}

#[test]
fn test_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let registry = Arc::new(RouteRegistry::new());
    let mut handles = vec![];

    for i in 0..10 {
        let reg = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            let route = make_route(&format!("thread-{i}.localhost"), 3000 + i as u16);
            reg.register(route).unwrap();
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(registry.list().len(), 10);
}
