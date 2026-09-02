use serde::{Deserialize, Serialize};

/// IPC protocol version
pub const PROTOCOL_VERSION: u32 = 1;

/// A message sent between CLI and daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcMessage {
    pub version: u32,
    pub payload: IpcPayload,
}

impl IpcMessage {
    pub fn new(payload: IpcPayload) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            payload,
        }
    }
}

/// The payload of an IPC message (command or response)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcPayload {
    // Commands (CLI → Daemon)
    RegisterRoute(RegisterRouteRequest),
    UnregisterRoute(UnregisterRouteRequest),
    ListRoutes,
    Ping,
    Shutdown,

    // Responses (Daemon → CLI)
    Ok(OkResponse),
    Error(ErrorResponse),
    RoutesList(RoutesListResponse),
    Pong,
    Status(StatusResponse),
}

/// Request to register a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRouteRequest {
    pub domain: String,
    pub port: u16,
    pub pid: Option<u32>,
}

/// Request to unregister a route
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnregisterRouteRequest {
    pub domain: String,
}

/// Generic OK response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OkResponse {
    pub message: String,
}

/// Error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

/// List of routes response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutesListResponse {
    pub routes: Vec<RouteInfo>,
}

/// Route info for IPC (serializable version of Route)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteInfo {
    pub domain: String,
    pub port: u16,
    pub pid: Option<u32>,
    pub created_at_secs: u64,
}

/// Daemon status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub pid: u32,
    pub uptime_secs: u64,
    pub route_count: usize,
    pub socket_path: String,
}
