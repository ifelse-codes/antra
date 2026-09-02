use antra::ipc::protocol::*;

#[test]
fn test_ipc_message_new_sets_version() {
    let msg = IpcMessage::new(IpcPayload::Ping);
    assert_eq!(msg.version, PROTOCOL_VERSION);
    assert!(matches!(msg.payload, IpcPayload::Ping));
}

#[test]
fn test_protocol_version_is_one() {
    assert_eq!(PROTOCOL_VERSION, 1);
}

#[test]
fn test_ping_pong_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::Ping);
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.version, 1);
    assert!(matches!(decoded.payload, IpcPayload::Ping));
}

#[test]
fn test_register_route_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::RegisterRoute(RegisterRouteRequest {
        domain: "myapp.localhost".to_string(),
        port: 5173,
        pid: Some(1234),
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::RegisterRoute(req) => {
            assert_eq!(req.domain, "myapp.localhost");
            assert_eq!(req.port, 5173);
            assert_eq!(req.pid, Some(1234));
        }
        _ => panic!("Expected RegisterRoute"),
    }
}

#[test]
fn test_unregister_route_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
        domain: "test.localhost".to_string(),
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::UnregisterRoute(req) => {
            assert_eq!(req.domain, "test.localhost");
        }
        _ => panic!("Expected UnregisterRoute"),
    }
}

#[test]
fn test_list_routes_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::ListRoutes);
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    assert!(matches!(decoded.payload, IpcPayload::ListRoutes));
}

#[test]
fn test_shutdown_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::Shutdown);
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    assert!(matches!(decoded.payload, IpcPayload::Shutdown));
}

#[test]
fn test_ok_response_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::Ok(OkResponse {
        message: "done".to_string(),
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::Ok(resp) => assert_eq!(resp.message, "done"),
        _ => panic!("Expected Ok"),
    }
}

#[test]
fn test_error_response_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::Error(ErrorResponse {
        message: "something failed".to_string(),
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::Error(resp) => assert_eq!(resp.message, "something failed"),
        _ => panic!("Expected Error"),
    }
}

#[test]
fn test_routes_list_response_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::RoutesList(RoutesListResponse {
        routes: vec![
            RouteInfo {
                domain: "a.localhost".to_string(),
                port: 3000,
                pid: None,
                created_at_secs: 10,
            },
            RouteInfo {
                domain: "b.localhost".to_string(),
                port: 4000,
                pid: Some(5678),
                created_at_secs: 60,
            },
        ],
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::RoutesList(list) => {
            assert_eq!(list.routes.len(), 2);
            assert_eq!(list.routes[0].domain, "a.localhost");
            assert_eq!(list.routes[1].port, 4000);
            assert_eq!(list.routes[1].pid, Some(5678));
        }
        _ => panic!("Expected RoutesList"),
    }
}

#[test]
fn test_status_response_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::Status(StatusResponse {
        pid: 42,
        uptime_secs: 3600,
        route_count: 5,
        socket_path: "/tmp/antra/daemon.sock".to_string(),
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::Status(status) => {
            assert_eq!(status.pid, 42);
            assert_eq!(status.uptime_secs, 3600);
            assert_eq!(status.route_count, 5);
            assert_eq!(status.socket_path, "/tmp/antra/daemon.sock");
        }
        _ => panic!("Expected Status"),
    }
}

#[test]
fn test_serde_tag_discriminant() {
    let msg = IpcMessage::new(IpcPayload::Ping);
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains(r#""type":"Ping""#));
}

#[test]
fn test_empty_domain_roundtrip() {
    let msg = IpcMessage::new(IpcPayload::RegisterRoute(RegisterRouteRequest {
        domain: String::new(),
        port: 0,
        pid: None,
    }));
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: IpcMessage = serde_json::from_str(&json).unwrap();

    match decoded.payload {
        IpcPayload::RegisterRoute(req) => {
            assert_eq!(req.domain, "");
            assert_eq!(req.port, 0);
            assert_eq!(req.pid, None);
        }
        _ => panic!("Expected RegisterRoute"),
    }
}

#[test]
fn test_clone_preserves_data() {
    let msg = IpcMessage::new(IpcPayload::Ok(OkResponse {
        message: "test".to_string(),
    }));
    let cloned = msg.clone();

    assert_eq!(msg.version, cloned.version);
    match (&msg.payload, &cloned.payload) {
        (IpcPayload::Ok(a), IpcPayload::Ok(b)) => assert_eq!(a.message, b.message),
        _ => panic!("Payload mismatch"),
    }
}
