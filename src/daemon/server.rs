use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::{watch, RwLock};

use crate::certs::cache::CertCache;
use crate::ipc::protocol::StartupStatus;
use crate::ipc::server::pid_path;
#[cfg(unix)]
use crate::ipc::server::socket_path;
use crate::routing::registry::RouteRegistry;
use crate::routing::types::{Protocol, Route};

/// Default idle timeout (10 minutes)
const DEFAULT_IDLE_TIMEOUT: Duration = Duration::from_secs(600);

/// Daemon configuration
pub struct DaemonConfig {
    pub https_port: u16,
    pub http_port: u16,
    pub idle_timeout: Duration,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            https_port: 443,
            http_port: 80,
            idle_timeout: DEFAULT_IDLE_TIMEOUT,
        }
    }
}

/// Probe and bind the HTTPS + HTTP ports (with fallbacks), spawning the
/// proxy servers. Runs BEFORE the daemon claims the pid file / IPC socket so
/// a port-blocked start fails loudly instead of half-starting behind the
/// singleton socket (all routes would 502).
///
/// Returns `(https_port, https_ok, https_error, http_port, http_ok, http_error)`.
/// Callers must refuse to start when `!https_ok` — HTTPS is the product; an
/// HTTP-only redirect daemon serves nothing.
async fn bind_proxy_ports(
    config: &DaemonConfig,
    registry: &Arc<RouteRegistry>,
    cert_cache: &Arc<CertCache>,
) -> (u16, bool, Option<String>, u16, bool, Option<String>) {
    // Probe HTTPS port first: the HTTP→HTTPS redirect needs the actual HTTPS
    // port for its Location header (it differs on fallback, e.g. 8443).
    let https_port = config.https_port;
    let actual_https_port;
    let https_ok;
    let https_error;

    if crate::proxy::https::probe_port(https_port).await.is_ok() {
        actual_https_port = https_port;
        https_ok = true;
        https_error = None;
        let https_registry = Arc::clone(registry);
        let https_cert_cache = Arc::clone(cert_cache);
        let port = https_port;
        tokio::spawn(async move {
            if let Err(e) =
                crate::proxy::https::start_server(port, https_registry, https_cert_cache).await
            {
                tracing::error!(error = %e, "HTTPS server failed");
            }
        });
    } else {
        let fallback = match https_port {
            443 => 8443,
            p => p + 1000,
        };
        tracing::warn!(
            port = https_port,
            "HTTPS port in use, trying fallback {}",
            fallback
        );
        if crate::proxy::https::probe_port(fallback).await.is_ok() {
            actual_https_port = fallback;
            https_ok = true;
            https_error = None;
            let reg = Arc::clone(registry);
            let cache = Arc::clone(cert_cache);
            tokio::spawn(async move {
                if let Err(e) = crate::proxy::https::start_server(fallback, reg, cache).await {
                    tracing::error!(error = %e, "HTTPS fallback server failed");
                }
            });
        } else {
            actual_https_port = fallback;
            https_ok = false;
            https_error = Some(format!("Both {https_port} and {fallback} are in use"));
        }
        // Show helpful message about what's using the port
        if let Some(hint) = crate::util::port::describe_port_conflict(https_port) {
            tracing::info!(port = https_port, "{}", hint);
        }
    }

    // Probe HTTP port and start with auto-fallback
    let http_port = config.http_port;
    let actual_http_port;
    let http_ok;
    let http_error;

    if crate::proxy::https::probe_port(http_port).await.is_ok() {
        actual_http_port = http_port;
        http_ok = true;
        http_error = None;
        if let Ok(listeners) = crate::proxy::https::bind_http_redirect(http_port).await {
            for l in listeners {
                crate::proxy::https::run_http_redirect(l, actual_https_port);
            }
        }
    } else {
        let fallback = match http_port {
            80 => 8080,
            p => p + 1000,
        };
        tracing::warn!(
            port = http_port,
            "HTTP port in use, trying fallback {}",
            fallback
        );
        if crate::proxy::https::probe_port(fallback).await.is_ok() {
            actual_http_port = fallback;
            http_ok = true;
            http_error = None;
            if let Ok(listeners) = crate::proxy::https::bind_http_redirect(fallback).await {
                for l in listeners {
                    crate::proxy::https::run_http_redirect(l, actual_https_port);
                }
            }
        } else {
            actual_http_port = fallback;
            http_ok = false;
            http_error = Some(format!("Both {http_port} and {fallback} are in use"));
        }
        // Show helpful message about what's using the port
        if let Some(hint) = crate::util::port::describe_port_conflict(http_port) {
            tracing::info!(port = http_port, "{}", hint);
        }
    }

    (
        actual_https_port,
        https_ok,
        https_error,
        actual_http_port,
        http_ok,
        http_error,
    )
}

/// Start the daemon process
pub async fn start_daemon(config: DaemonConfig) -> Result<()> {
    let pid_file = pid_path();

    #[cfg(unix)]
    let sock_path = socket_path();

    // Check if already running
    #[cfg(unix)]
    if sock_path.exists() {
        // Try to connect to see if it's actually running
        if crate::ipc::client::is_daemon_running() {
            anyhow::bail!("Daemon is already running. Stop it first with: antra proxy stop");
        }
        // Stale socket, remove it
        std::fs::remove_file(&sock_path)?;
    }

    // PID-file singleton gate: a live daemon whose socket is gone (deleted
    // by an unconditional cleanup-on-error, `clean`, or a manual rm) still
    // holds its ports but is invisible to the socket check above. Starting
    // a second daemon then fails its port probes, half-starts, and steals
    // the socket — the split-brain. Refuse while the recorded PID is alive;
    // reap a stale pid file otherwise.
    #[cfg(unix)]
    if let Some(pid) = crate::ipc::server::is_daemon_pid_alive() {
        anyhow::bail!(
            "Daemon is already running (PID {pid}). Stop it first with: antra proxy stop"
        );
    } else if crate::ipc::server::read_daemon_pid().is_some() {
        let _ = std::fs::remove_file(&pid_file);
    }

    // Create socket directory
    #[cfg(unix)]
    if let Some(parent) = sock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    #[cfg(windows)]
    {
        let pid_dir = pid_file.parent().unwrap_or(std::path::Path::new("."));
        std::fs::create_dir_all(pid_dir)?;
    }

    // Initialize registry + certs BEFORE claiming anything (pid file, IPC
    // socket): a daemon that cannot initialize must fail without taking
    // the singleton socket from a healthy predecessor.
    let registry = Arc::new(RouteRegistry::new());
    let start_time = Instant::now();

    // Track last activity time for idle shutdown
    let last_activity = Arc::new(RwLock::new(Instant::now()));

    // Initialize cert cache
    let cert_cache = Arc::new(
        CertCache::new().map_err(|e| anyhow::anyhow!("Failed to initialize cert cache: {e}"))?,
    );

    // Probe + bind the proxy ports BEFORE the IPC socket: a daemon that
    // cannot serve HTTPS must fail loudly instead of half-starting and
    // stealing the socket (all routes would 502 behind it).
    let (actual_https_port, https_ok, https_error, actual_http_port, http_ok, http_error) =
        bind_proxy_ports(&config, &registry, &cert_cache).await;
    if !https_ok {
        anyhow::bail!(
            "Cannot start daemon: HTTPS unavailable ({}). Not taking the IPC socket — free the port and retry.",
            https_error.unwrap_or_default()
        );
    }

    // Write PID file
    std::fs::write(&pid_file, std::process::id().to_string())?;

    // Create the IPC listener (the atomic singleton claim: exactly one
    // starter wins the bind; the loser fails here with EADDRINUSE)
    #[cfg(unix)]
    {
        // Remove old socket if it exists
        let _ = std::fs::remove_file(&sock_path);
    }

    #[cfg(unix)]
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    // Set permissions on socket (owner read/write only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&sock_path, PermissionsExt::from_mode(0o600));
    }

    // Restore static aliases persisted by `antra alias` / `antra add route`
    // so they survive daemon restarts. Managed `run`/`dev` routes carry
    // `managed=true` and never reach disk, so only unmanaged entries load.
    let restored = crate::routing::persist::load_aliases();
    for entry in &restored {
        let _ = registry.register(Route {
            domain: entry.domain.clone(),
            host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
            port: entry.port,
            pid: None,
            managed: false,
            protocol: Protocol::Http,
            created_at: Instant::now(),
        });
    }
    if !restored.is_empty() {
        tracing::info!(count = restored.len(), "Restored persisted static aliases");
    }

    tracing::info!(pid = std::process::id(), "Daemon starting");

    // Set up shutdown signal
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);

    // Initialize global shutdown signal for IPC
    crate::ipc::server::init_shutdown(shutdown_tx.clone());

    // Spawn idle timeout checker
    let idle_registry = Arc::clone(&registry);
    let idle_timeout = config.idle_timeout;
    let idle_tx = shutdown_tx.clone();
    let idle_activity = Arc::clone(&last_activity);
    tokio::spawn(async move {
        let mut check_interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            check_interval.tick().await;

            // Only check idle shutdown if we have the timeout configured
            if idle_timeout.as_secs() == 0 {
                continue;
            }

            let routes = idle_registry.list();
            let last = idle_activity.read().await;

            // If no routes and idle timeout exceeded, shut down
            if routes.is_empty() && last.elapsed() >= idle_timeout {
                tracing::info!(
                    idle_secs = last.elapsed().as_secs(),
                    timeout_secs = idle_timeout.as_secs(),
                    "Idle timeout reached with no routes, shutting down"
                );
                let _ = idle_tx.send(true);
                break;
            }
        }
    });

    // Store startup status for IPC queries
    let startup_status = Arc::new(tokio::sync::Mutex::new(StartupStatus {
        https_port: actual_https_port,
        https_ok,
        https_error,
        http_port: actual_http_port,
        http_ok,
        http_error,
    }));

    // Set global startup status for IPC queries
    crate::ipc::server::set_startup_status(Arc::clone(&startup_status));

    // Spawn signal handler for graceful shutdown
    let signal_tx = shutdown_tx;
    #[cfg(unix)]
    tokio::spawn(async move {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        let mut sigint =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt()).unwrap();

        tokio::select! {
            _ = sigterm.recv() => {
                tracing::info!("Received SIGTERM");
            }
            _ = sigint.recv() => {
                tracing::info!("Received SIGINT");
            }
        }
        let _ = signal_tx.send(true);
    });

    #[cfg(windows)]
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            tracing::info!("Received Ctrl+C");
            let _ = signal_tx.send(true);
        }
    });

    // Start IPC server (this blocks)
    // Log the ports actually bound (fallbacks included), not the requested ones.
    tracing::info!(
        https_port = actual_https_port,
        https_ok,
        http_port = actual_http_port,
        http_ok,
        idle_timeout_secs = config.idle_timeout.as_secs(),
        "Daemon ready"
    );

    // Run IPC server and wait for shutdown
    #[cfg(unix)]
    tokio::select! {
        result = crate::ipc::server::start_ipc_server(listener, Arc::clone(&registry), start_time, Arc::clone(&last_activity)) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "IPC server error");
            }
        }
        _ = shutdown_rx.changed() => {
            tracing::info!("Shutdown signal received");
        }
    }

    #[cfg(windows)]
    tokio::select! {
        result = crate::ipc::server::start_ipc_server(Arc::clone(&registry), start_time, Arc::clone(&last_activity)) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "IPC server error");
            }
        }
        _ = shutdown_rx.changed() => {
            tracing::info!("Shutdown signal received");
        }
    }

    // Cleanup
    #[cfg(unix)]
    let _ = std::fs::remove_file(&sock_path);
    let _ = std::fs::remove_file(&pid_file);

    tracing::info!("Daemon stopped");
    Ok(())
}

/// Stop a running daemon
pub fn stop_daemon() -> Result<()> {
    let pid_file = pid_path();

    #[cfg(unix)]
    let sock_path = socket_path();

    #[cfg(unix)]
    if !sock_path.exists() {
        // No socket — but the daemon may still be alive without one
        // (socketless). Only reap the pid file when its PID is dead;
        // never delete a live daemon's last proof of existence.
        if let Some(pid) = crate::ipc::server::is_daemon_pid_alive() {
            anyhow::bail!(
                "Daemon (PID {pid}) seems to be running without a socket. Stop it with `kill {pid}`, then retry."
            );
        }
        let _ = std::fs::remove_file(&pid_file);
        anyhow::bail!("Daemon is not running");
    }

    #[cfg(windows)]
    if !crate::ipc::client::is_daemon_running() {
        let _ = std::fs::remove_file(&pid_file);
        anyhow::bail!("Daemon is not running");
    }

    // Send shutdown command
    match crate::ipc::client::send_command_sync(crate::ipc::protocol::IpcPayload::Shutdown) {
        Ok(_) => {
            // Wait a moment for daemon to exit
            std::thread::sleep(Duration::from_millis(500));

            // Clean up files
            #[cfg(unix)]
            let _ = std::fs::remove_file(&sock_path);
            let _ = std::fs::remove_file(&pid_file);

            Ok(())
        }
        Err(_e) => {
            // Connection failed. Only clean up when the recorded daemon is
            // actually dead: deleting the socket of a LIVE-but-unresponsive
            // daemon orphans it (it keeps its ports, goes invisible, and the
            // next start steals the socket — the split-brain).
            #[cfg(unix)]
            match crate::ipc::server::is_daemon_pid_alive() {
                Some(pid) => {
                    anyhow::bail!(
                        "Daemon (PID {pid}) is running but not responding — leaving its socket alone. If it is wedged, `kill {pid}` then retry."
                    )
                }
                None => {
                    let _ = std::fs::remove_file(&sock_path);
                    let _ = std::fs::remove_file(&pid_file);
                    anyhow::bail!("Daemon is not running");
                }
            }
            #[cfg(not(unix))]
            {
                let _ = std::fs::remove_file(&pid_file);
                anyhow::bail!("Daemon is not running");
            }
        }
    }
}

/// Get daemon status
pub fn daemon_status() -> Result<String> {
    crate::ipc::client::send_command_ok(crate::ipc::protocol::IpcPayload::Status(
        crate::ipc::protocol::StatusResponse {
            pid: 0,
            uptime_secs: 0,
            route_count: 0,
            socket_path: String::new(),
        },
    ))
}
