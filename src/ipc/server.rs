use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{watch, RwLock};

use super::protocol::*;
use crate::routing::registry::RouteRegistry;
use crate::routing::types::{Protocol, Route};

/// Global shutdown signal (set by daemon server)
static SHUTDOWN_TX: std::sync::OnceLock<watch::Sender<bool>> = std::sync::OnceLock::new();

/// Initialize the global shutdown signal
pub fn init_shutdown(tx: watch::Sender<bool>) {
    let _ = SHUTDOWN_TX.set(tx);
}

/// Signal shutdown to the daemon
pub fn signal_shutdown() {
    if let Some(tx) = SHUTDOWN_TX.get() {
        let _ = tx.send(true);
    }
}

/// Path to the daemon socket (Unix only)
#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    let dir = dirs::runtime_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join("antra").join("daemon.sock")
}

/// Path to the daemon PID file
pub fn pid_path() -> PathBuf {
    #[cfg(unix)]
    {
        let dir = dirs::runtime_dir()
            .or_else(dirs::data_local_dir)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        dir.join("antra").join("daemon.pid")
    }
    #[cfg(windows)]
    {
        let dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        dir.join("antra").join("daemon.pid")
    }
}

/// Windows named pipe path
#[cfg(windows)]
pub fn pipe_path() -> String {
    r"\\.\pipe\antra-daemon".to_string()
}

/// Handle a single IPC message from a reader/writer pair
async fn handle_one_message(
    reader: &mut (impl tokio::io::AsyncBufRead + Unpin),
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    registry: &Arc<RouteRegistry>,
    start_time: Instant,
    last_activity: &Arc<RwLock<Instant>>,
) -> Result<()> {
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    if line.is_empty() {
        return Ok(());
    }

    let msg: IpcMessage = match serde_json::from_str(line.trim()) {
        Ok(m) => m,
        Err(e) => {
            let resp = IpcMessage::new(IpcPayload::Error(ErrorResponse {
                message: format!("Invalid message: {e}"),
            }));
            send_response(writer, &resp).await?;
            return Ok(());
        }
    };

    if msg.version != PROTOCOL_VERSION {
        let resp = IpcMessage::new(IpcPayload::Error(ErrorResponse {
            message: format!(
                "Protocol version mismatch: got {}, expected {PROTOCOL_VERSION}",
                msg.version
            ),
        }));
        send_response(writer, &resp).await?;
        return Ok(());
    }

    *last_activity.write().await = Instant::now();

    let response = match msg.payload {
        IpcPayload::RegisterRoute(req) => handle_register_route(req, registry),
        IpcPayload::UnregisterRoute(req) => handle_unregister_route(req, registry),
        IpcPayload::ListRoutes => handle_list_routes(registry),
        IpcPayload::Ping => IpcMessage::new(IpcPayload::Pong),
        IpcPayload::Shutdown => {
            let resp = IpcMessage::new(IpcPayload::Ok(OkResponse {
                message: "Shutting down".to_string(),
            }));
            send_response(writer, &resp).await?;
            signal_shutdown();
            return Ok(());
        }
        IpcPayload::Status(_) => handle_status(start_time, registry),
        _ => IpcMessage::new(IpcPayload::Error(ErrorResponse {
            message: "Unknown command".to_string(),
        })),
    };

    send_response(writer, &response).await?;
    Ok(())
}

// Unix domain socket implementation
#[cfg(unix)]
pub mod unix_server {
    use super::*;
    use tokio::net::UnixListener;

    pub async fn start_ipc_server(
        listener: UnixListener,
        registry: Arc<RouteRegistry>,
        start_time: Instant,
        last_activity: Arc<RwLock<Instant>>,
    ) -> Result<()> {
        tracing::info!("IPC server listening (Unix socket)");

        loop {
            let (stream, _addr) = listener.accept().await?;
            let registry = Arc::clone(&registry);
            let last_activity = Arc::clone(&last_activity);

            tokio::spawn(async move {
                let (read_half, mut write_half) = stream.into_split();
                let mut reader = BufReader::new(read_half);

                if let Err(e) = handle_one_message(
                    &mut reader,
                    &mut write_half,
                    &registry,
                    start_time,
                    &last_activity,
                )
                .await
                {
                    tracing::error!(error = %e, "IPC connection error");
                }
            });
        }
    }
}

// Windows named pipe implementation
#[cfg(windows)]
pub mod windows_server {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    pub async fn start_ipc_server(
        registry: Arc<RouteRegistry>,
        start_time: Instant,
        last_activity: Arc<RwLock<Instant>>,
    ) -> Result<()> {
        let pipe_name = pipe_path();
        tracing::info!(pipe = %pipe_name, "IPC server listening (Windows named pipe)");

        loop {
            let server = ServerOptions::new()
                .first_pipe_instance(false)
                .create(&pipe_name)?;

            server.connect().await?;

            let registry = Arc::clone(&registry);
            let last_activity = Arc::clone(&last_activity);

            tokio::spawn(async move {
                // NamedPipeServer implements AsyncRead for &NamedPipeServer
                // so we need &server to call read (via AsyncReadExt)
                let mut reader = &server;
                let mut buf = vec![0u8; 4096];
                let n = match tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await {
                    Ok(n) if n > 0 => n,
                    Ok(_) => return,
                    Err(e) => {
                        tracing::error!(error = %e, "IPC read error");
                        return;
                    }
                };

                let line = match String::from_utf8(buf[..n].to_vec()) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::error!(error = %e, "IPC invalid UTF-8");
                        return;
                    }
                };

                if line.is_empty() {
                    return;
                }

                let msg: IpcMessage = match serde_json::from_str(line.trim()) {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::error!(error = %e, "IPC invalid JSON");
                        return;
                    }
                };

                if msg.version != PROTOCOL_VERSION {
                    tracing::error!("IPC protocol version mismatch");
                    return;
                }

                *last_activity.write().await = Instant::now();

                let response = match msg.payload {
                    IpcPayload::RegisterRoute(req) => handle_register_route(req, &registry),
                    IpcPayload::UnregisterRoute(req) => handle_unregister_route(req, &registry),
                    IpcPayload::ListRoutes => handle_list_routes(&registry),
                    IpcPayload::Ping => IpcMessage::new(IpcPayload::Pong),
                    IpcPayload::Shutdown => {
                        let resp = IpcMessage::new(IpcPayload::Ok(OkResponse {
                            message: "Shutting down".to_string(),
                        }));
                        // Best effort write using &server (AsyncWrite for &NamedPipeServer)
                        let json = serde_json::to_string(&resp).unwrap_or_default();
                        let mut writer = &server;
                        let _ = tokio::io::AsyncWriteExt::write_all(&mut writer, json.as_bytes()).await;
                        let _ = tokio::io::AsyncWriteExt::write_all(&mut writer, b"\n").await;
                        signal_shutdown();
                        return;
                    }
                    IpcPayload::Status(_) => handle_status(start_time, &registry),
                    _ => IpcMessage::new(IpcPayload::Error(ErrorResponse {
                        message: "Unknown command".to_string(),
                    })),
                };

                let json = serde_json::to_string(&response).unwrap_or_default();
                let mut writer = &server;
                let _ = tokio::io::AsyncWriteExt::write_all(&mut writer, json.as_bytes()).await;
                let _ = tokio::io::AsyncWriteExt::write_all(&mut writer, b"\n").await;
            });
        }
    }
}

#[cfg(unix)]
pub use unix_server::start_ipc_server;

#[cfg(windows)]
pub use windows_server::start_ipc_server;

fn handle_register_route(req: RegisterRouteRequest, registry: &RouteRegistry) -> IpcMessage {
    let route = Route {
        domain: req.domain.clone(),
        host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        port: req.port,
        pid: req.pid,
        protocol: Protocol::Http,
        created_at: Instant::now(),
    };

    match registry.register(route) {
        Ok(()) => IpcMessage::new(IpcPayload::Ok(OkResponse {
            message: format!("Route registered: {} → 127.0.0.1:{}", req.domain, req.port),
        })),
        Err(e) => IpcMessage::new(IpcPayload::Error(ErrorResponse {
            message: format!("Failed to register route: {e}"),
        })),
    }
}

fn handle_unregister_route(req: UnregisterRouteRequest, registry: &RouteRegistry) -> IpcMessage {
    match registry.unregister(&req.domain) {
        Ok(()) => IpcMessage::new(IpcPayload::Ok(OkResponse {
            message: format!("Route unregistered: {}", req.domain),
        })),
        Err(e) => IpcMessage::new(IpcPayload::Error(ErrorResponse {
            message: format!("Failed to unregister route: {e}"),
        })),
    }
}

fn handle_list_routes(registry: &RouteRegistry) -> IpcMessage {
    let routes = registry
        .list()
        .into_iter()
        .map(|r| RouteInfo {
            domain: r.domain,
            port: r.port,
            pid: r.pid,
            created_at_secs: r.created_at.elapsed().as_secs(),
        })
        .collect();

    IpcMessage::new(IpcPayload::RoutesList(RoutesListResponse { routes }))
}

fn handle_status(start_time: Instant, registry: &RouteRegistry) -> IpcMessage {
    #[cfg(unix)]
    let ipc_path = socket_path().to_string_lossy().into_owned();
    #[cfg(windows)]
    let ipc_path = pipe_path();

    IpcMessage::new(IpcPayload::Status(StatusResponse {
        pid: std::process::id(),
        uptime_secs: start_time.elapsed().as_secs(),
        route_count: registry.list().len(),
        socket_path: ipc_path,
    }))
}

async fn send_response(
    writer: &mut (impl tokio::io::AsyncWrite + Unpin),
    msg: &IpcMessage,
) -> Result<()> {
    let json = serde_json::to_string(msg)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}
