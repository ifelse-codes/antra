use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
#[cfg(unix)]
use tokio::io::BufReader;
#[allow(unused_imports)]
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt};
use tokio::sync::{watch, RwLock};

use super::protocol::*;
use crate::routing::registry::RouteRegistry;
use crate::routing::types::{Protocol, Route};

/// Global shutdown signal (set by daemon server)
static SHUTDOWN_TX: std::sync::OnceLock<watch::Sender<bool>> = std::sync::OnceLock::new();

/// Global startup status (set by daemon server)
static STARTUP_STATUS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Mutex<StartupStatus>>> =
    std::sync::OnceLock::new();

/// Initialize the global shutdown signal
pub fn init_shutdown(tx: watch::Sender<bool>) {
    let _ = SHUTDOWN_TX.set(tx);
}

/// Set the global startup status
pub fn set_startup_status(status: Arc<tokio::sync::Mutex<StartupStatus>>) {
    let _ = STARTUP_STATUS.set(status);
}

/// Fingerprint of the CA this daemon is serving, set once at startup.
///
/// Process-global for the same reason `STARTUP_STATUS` is: the handler that
/// answers `Status` has no handle to the certificate cache, and threading one
/// through every layer of the IPC path would buy nothing.
static CA_FINGERPRINT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Publish the CA fingerprint reported by `Status` queries.
pub fn set_ca_fingerprint(fingerprint: String) {
    let _ = CA_FINGERPRINT.set(fingerprint);
}

/// Signal shutdown to the daemon
pub fn signal_shutdown() {
    if let Some(tx) = SHUTDOWN_TX.get() {
        let _ = tx.send(true);
    }
}

/// Longest byte length a `sockaddr_un::sun_path` can hold, excluding the NUL.
///
/// 104 on macOS, 108 on Linux. A bind with anything longer does not fail
/// cleanly — `libc` rejects it, and the daemon dies with the bare message
/// `path must be shorter than SUN_LEN`, naming neither the path nor the
/// limit. `tests/common/mod.rs` already documents this by working around it.
#[cfg(unix)]
pub const SUN_PATH_MAX: usize = if cfg!(target_os = "macos") { 104 } else { 108 };

/// Path to the daemon socket (Unix only)
#[cfg(unix)]
pub fn socket_path() -> PathBuf {
    let dir = dirs::runtime_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let path = dir.join("antra").join("daemon.sock");
    if fits_sun_path(&path) {
        return path;
    }
    // Too long for a `sun_path`. Fall back to a short, still-per-user path:
    // on macOS `data_local_dir()` is `$HOME/Library/Application Support`, a
    // 50-character fixed overhead that leaves under 54 for `$HOME` itself, so
    // any home directory longer than that — a long username, or a CI runner's
    // `mktemp -d` — used to make the daemon unstartable with no way out.
    // Folding the original path into a hash keeps distinct homes (and so
    // distinct daemons) from colliding on the one short name.
    // The user's uid, not root's, under `sudo`: the `proxy start` launcher
    // stays root while the daemon it starts drops to the user (C27), and
    // both — and the user's own CLI — must arrive at the same socket.
    let uid = crate::platform::acting_uid().unwrap_or_else(|| unsafe { libc::geteuid() });
    let hash = crate::certs::fingerprint(path.to_string_lossy().as_bytes());
    PathBuf::from("/tmp")
        .join(format!("antra-{uid}"))
        .join(hash)
        .join("d.sock")
}

/// Whether `path` is short enough to bind as a Unix socket.
#[cfg(unix)]
fn fits_sun_path(path: &std::path::Path) -> bool {
    path.as_os_str().len() < SUN_PATH_MAX
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

/// Parse pid-file contents into a PID. Pure (no fs) so it is unit-testable;
/// fs + liveness live in `read_daemon_pid` / `is_daemon_pid_alive` below.
#[cfg_attr(windows, allow(dead_code))]
pub(crate) fn parse_pid_contents(contents: &str) -> Option<u32> {
    contents.trim().parse::<u32>().ok().filter(|&pid| pid > 0)
}

/// Read the daemon PID file, if present and parseable. Returns None when the
/// file is missing or corrupt — callers must treat that as "unknown", never
/// as "dead" on its own.
#[cfg_attr(windows, allow(dead_code))]
pub fn read_daemon_pid() -> Option<u32> {
    std::fs::read_to_string(pid_path())
        .ok()
        .and_then(|s| parse_pid_contents(&s))
}

/// The PID recorded in the pid file, if that process is currently alive.
/// Unix only: elsewhere there is no reliable signal-0 equivalent here, so
/// those platforms keep the previous socket-only behavior.
#[cfg(unix)]
pub fn is_daemon_pid_alive() -> Option<u32> {
    let pid = read_daemon_pid()?;
    pid_is_alive(pid).then_some(pid)
}

/// Signal-0 liveness probe for an arbitrary PID. Shared by the daemon
/// singleton gate, stop/clean recovery, and (via their own copies, kept for
/// minimal churn) prune/run/proxy/doctor.
#[cfg(unix)]
pub fn pid_is_alive(pid: u32) -> bool {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok()
}

/// Windows named pipe path
#[cfg(windows)]
pub fn pipe_path() -> String {
    r"\\.\pipe\antra-daemon".to_string()
}

/// Handle a single IPC message from a reader/writer pair
#[cfg_attr(windows, allow(dead_code))]
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
        IpcPayload::GetStartupStatus => handle_get_startup_status().await,
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
    #[allow(unused_imports)]
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// Parallel accept workers so concurrent clients never see
    /// `ERROR_PIPE_BUSY` (os error 231). A single create→connect loop offers
    /// only one pending instance: a liveness probe holding its connection
    /// (open, no write yet) starves the real command behind it, and
    /// `is_daemon_running` falsely reported "not running".
    const ACCEPT_WORKERS: usize = 4;

    pub async fn start_ipc_server(
        registry: Arc<RouteRegistry>,
        start_time: Instant,
        last_activity: Arc<RwLock<Instant>>,
    ) -> Result<()> {
        let pipe_name = pipe_path();
        tracing::info!(pipe = %pipe_name, "IPC server listening (Windows named pipe)");

        for _ in 0..ACCEPT_WORKERS {
            let pipe_name = pipe_name.clone();
            let registry = Arc::clone(&registry);
            let last_activity = Arc::clone(&last_activity);
            tokio::spawn(async move {
                loop {
                    let server = match ServerOptions::new()
                        .first_pipe_instance(false)
                        .create(&pipe_name)
                    {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::error!(error = %e, "IPC pipe create failed, retrying");
                            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            continue;
                        }
                    };
                    if let Err(e) = server.connect().await {
                        tracing::error!(error = %e, "IPC pipe connect failed");
                        continue;
                    }

                    let registry = Arc::clone(&registry);
                    let last_activity = Arc::clone(&last_activity);

                    tokio::spawn(async move {
                        handle_pipe_connection(server, &registry, start_time, &last_activity).await;
                    });
                }
            });
        }

        // Workers run until the daemon's shutdown select drops this future.
        std::future::pending::<()>().await;
        #[allow(unreachable_code)]
        Ok(())
    }

    async fn handle_pipe_connection(
        mut server: NamedPipeServer,
        registry: &Arc<RouteRegistry>,
        start_time: Instant,
        last_activity: &Arc<RwLock<Instant>>,
    ) {
        // AsyncRead is implemented for NamedPipeServer (owned), takes &mut self
        let mut buf = vec![0u8; 4096];
        let n = match server.read(&mut buf).await {
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
            IpcPayload::RegisterRoute(req) => handle_register_route(req, registry),
            IpcPayload::UnregisterRoute(req) => handle_unregister_route(req, registry),
            IpcPayload::ListRoutes => handle_list_routes(registry),
            IpcPayload::Ping => IpcMessage::new(IpcPayload::Pong),
            IpcPayload::Shutdown => {
                let resp = IpcMessage::new(IpcPayload::Ok(OkResponse {
                    message: "Shutting down".to_string(),
                }));
                let json = serde_json::to_string(&resp).unwrap_or_default();
                // AsyncWrite is implemented for NamedPipeServer (owned)
                let _ = server.write_all(json.as_bytes()).await;
                let _ = server.write_all(b"\n").await;
                signal_shutdown();
                return;
            }
            IpcPayload::Status(_) => handle_status(start_time, registry),
            IpcPayload::GetStartupStatus => handle_get_startup_status().await,
            _ => IpcMessage::new(IpcPayload::Error(ErrorResponse {
                message: "Unknown command".to_string(),
            })),
        };

        let json = serde_json::to_string(&response).unwrap_or_default();
        // AsyncWrite is implemented for NamedPipeServer (owned)
        let _ = server.write_all(json.as_bytes()).await;
        let _ = server.write_all(b"\n").await;
    }
}

#[cfg(unix)]
pub use unix_server::start_ipc_server;

#[cfg(windows)]
pub use windows_server::start_ipc_server;

fn handle_register_route(req: RegisterRouteRequest, registry: &RouteRegistry) -> IpcMessage {
    tracing::debug!(domain = %req.domain, port = req.port, "Registering route");
    let route = Route {
        domain: req.domain.clone(),
        host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
        port: req.port,
        pid: req.pid,
        managed: req.managed,
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
    tracing::debug!(domain = %req.domain, "Unregistering route");
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
            managed: r.managed,
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
        ca_fingerprint: CA_FINGERPRINT.get().cloned(),
    }))
}

async fn handle_get_startup_status() -> IpcMessage {
    match STARTUP_STATUS.get() {
        Some(status) => {
            let s = status.lock().await;
            IpcMessage::new(IpcPayload::StartupStatusResponse(s.clone()))
        }
        None => IpcMessage::new(IpcPayload::Error(ErrorResponse {
            message: "Startup status not available".to_string(),
        })),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_contents_parses_plain_pid() {
        assert_eq!(parse_pid_contents("12345"), Some(12345));
    }

    #[cfg(unix)]
    #[test]
    fn socket_path_always_fits_sun_path() {
        // The whole point of the guard: whatever `dirs` resolves, the returned
        // path must be bindable. Asserting on the real environment catches the
        // long-`$HOME` case that motivated it, on the machine that has it.
        let path = socket_path();
        assert!(
            fits_sun_path(&path),
            "socket path is {} bytes, over the {SUN_PATH_MAX}-byte limit: {}",
            path.as_os_str().len(),
            path.display()
        );
    }

    #[cfg(unix)]
    #[test]
    fn fits_sun_path_rejects_overlong_paths() {
        // A macOS-shaped home: 33 chars of "Library/Application Support"
        // plus "antra/daemon.sock" is 50 bytes of overhead before $HOME.
        let long_home = format!("/Users/{}/Library/Application Support", "a".repeat(60));
        let derived = std::path::Path::new(&long_home)
            .join("antra")
            .join("daemon.sock");
        assert!(derived.as_os_str().len() > SUN_PATH_MAX);
        assert!(!fits_sun_path(&derived));
        assert!(fits_sun_path(std::path::Path::new(
            "/tmp/antra/daemon.sock"
        )));
    }

    #[test]
    fn pid_contents_trims_whitespace_and_newline() {
        assert_eq!(parse_pid_contents("  12345\n"), Some(12345));
    }

    #[test]
    fn pid_contents_rejects_garbage() {
        assert_eq!(parse_pid_contents(""), None);
        assert_eq!(parse_pid_contents("not-a-pid\n"), None);
        assert_eq!(parse_pid_contents("0"), None);
        assert_eq!(parse_pid_contents("-7"), None);
    }

    #[test]
    fn own_process_counts_as_alive() {
        #[cfg(unix)]
        assert!(pid_is_alive(std::process::id()));
    }

    #[test]
    fn exited_child_counts_as_dead() {
        // A spawned-and-reaped child is deterministically dead (its PID is
        // free; nothing forks between wait() and the probe). Note: probing
        // u32::MAX is NOT a valid negative test — it wraps to pid -1, which
        // addresses every process and always reports alive.
        #[cfg(unix)]
        {
            let mut child = std::process::Command::new("true")
                .spawn()
                .expect("test needs a `true` binary on PATH");
            let pid = child.id();
            child.wait().unwrap();
            assert!(!pid_is_alive(pid));
        }
    }
}
