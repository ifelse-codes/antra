use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Result;
use tokio::sync::{watch, RwLock};

use crate::certs::cache::CertCache;
use crate::ipc::server::pid_path;
#[cfg(unix)]
use crate::ipc::server::socket_path;
use crate::routing::registry::RouteRegistry;

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

    // Write PID file
    std::fs::write(&pid_file, std::process::id().to_string())?;

    // Create the IPC listener
    #[cfg(unix)]
    {
        // Remove old socket if it exists
        let _ = std::fs::remove_file(&sock_path);
    }

    #[cfg(unix)]
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    // Set permissions on socket (owner read/write, others read/write)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&sock_path, PermissionsExt::from_mode(0o666));
    }

    let registry = Arc::new(RouteRegistry::new());
    let start_time = Instant::now();

    // Track last activity time for idle shutdown
    let last_activity = Arc::new(RwLock::new(Instant::now()));

    // Initialize cert cache
    let cert_cache = Arc::new(
        CertCache::new().map_err(|e| anyhow::anyhow!("Failed to initialize cert cache: {e}"))?,
    );

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

    // Start HTTP→HTTPS redirect
    let http_port = config.http_port;
    tokio::spawn(async move {
        if let Err(e) = crate::proxy::https::start_http_redirect(http_port).await {
            tracing::error!(error = %e, "HTTP redirect server error");
        }
    });

    // Start HTTPS server
    let https_registry = Arc::clone(&registry);
    let https_cert_cache = Arc::clone(&cert_cache);
    let https_port = config.https_port;
    tokio::spawn(async move {
        if let Err(e) =
            crate::proxy::https::start_server(https_port, https_registry, https_cert_cache).await
        {
            tracing::error!(error = %e, "HTTPS proxy server error");
        }
    });

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
    tracing::info!(
        https_port = config.https_port,
        http_port = config.http_port,
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
        // Try to clean up PID file anyway
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
        Err(e) => {
            // Force cleanup
            #[cfg(unix)]
            let _ = std::fs::remove_file(&sock_path);
            let _ = std::fs::remove_file(&pid_file);
            Err(anyhow::anyhow!("Failed to stop daemon: {e}"))
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
