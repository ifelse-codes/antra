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

/// How often the daemon looks for routes whose owner process has died.
/// Signal 0 per managed route on Unix; `tasklist` on Windows, hence not
/// every second.
const REAP_INTERVAL: Duration = Duration::from_secs(5);

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

    // Create the socket directory — private, and owned by the invoking user
    // under sudo — BEFORE anything is bound into it. A Unix socket file is
    // created at the process umask, so the `chmod 0o600` below the bind cannot
    // hold continuously: for as long as it is `0o755`, any local user can
    // connect and issue IPC commands. `connect` needs `x` on every component,
    // so a `0o755` socket inside this `0o700` directory is unreachable to
    // anyone else for the whole window. It also keeps the `/tmp/antra-<uid>/
    // <hash>/` fallback off the world-readable list.
    #[cfg(unix)]
    prepare_socket_dir(&sock_path)?;

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
    // Publish the fingerprint before binding anything: a CLI that asks
    // whether this daemon still holds the CA on disk needs an answer even
    // while routes are being served, and a daemon that rotated the CA at
    // startup must be identifiable as such.
    crate::ipc::server::set_ca_fingerprint(cert_cache.ca_fingerprint());
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
    // sudo-root daemon, user-owned HOME: hand the pid file back to the
    // invoking user so unprivileged `status`/`stop` can read it.
    #[cfg(unix)]
    crate::platform::chown_to_invoking_user(&pid_file);

    // Create the IPC listener (the atomic singleton claim: exactly one
    // starter wins the bind; the loser fails here with EADDRINUSE)
    #[cfg(unix)]
    {
        // Remove old socket if it exists
        let _ = std::fs::remove_file(&sock_path);
    }

    #[cfg(unix)]
    let listener = tokio::net::UnixListener::bind(&sock_path)?;

    // Set permissions on socket (owner read/write only). The bind above
    // created it at the process umask; `prepare_socket_dir` is what made that
    // harmless, and this is what holds for the daemon's whole lifetime.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&sock_path, PermissionsExt::from_mode(0o600));
        // sudo-root daemon, user-owned HOME: hand the socket back to the
        // invoking user so unprivileged CLI can connect (0600 still holds,
        // now enforced for the user instead of root).
        crate::platform::chown_to_invoking_user(&sock_path);
    }

    // Restore persisted routes so they survive daemon restarts.
    // - Static aliases (`managed=false`) always restore.
    // - Managed `run`/`dev` routes restore only when their PID is still
    //   alive; stale entries (dead owner) are dropped and pruned from disk
    //   by the re-persist triggered below.
    let restored = crate::routing::persist::load_aliases();
    let mut restored_count = 0usize;
    let mut dropped_stale = 0usize;
    for entry in &restored {
        if entry.managed {
            match entry.pid {
                Some(pid) if crate::platform::is_pid_alive(pid) => {
                    let _ = registry.register(Route {
                        domain: entry.domain.clone(),
                        host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                        port: entry.port,
                        pid: entry.pid,
                        managed: true,
                        protocol: Protocol::Http,
                        created_at: Instant::now(),
                    });
                    restored_count += 1;
                }
                _ => {
                    dropped_stale += 1;
                    tracing::info!(
                        domain = %entry.domain,
                        pid = ?entry.pid,
                        "Dropping stale managed route (owner dead)"
                    );
                }
            }
        } else {
            let _ = registry.register(Route {
                domain: entry.domain.clone(),
                host: std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST),
                port: entry.port,
                pid: None,
                managed: false,
                protocol: Protocol::Http,
                created_at: Instant::now(),
            });
            restored_count += 1;
        }
    }
    if restored_count > 0 || dropped_stale > 0 {
        tracing::info!(
            restored = restored_count,
            dropped_stale,
            "Restored persisted routes"
        );
    }
    // Prune stale managed entries from disk when nothing was re-registered
    // (all stale) — otherwise the dead PIDs would resurrect on every start.
    if dropped_stale > 0 {
        let live: Vec<crate::routing::persist::AliasEntry> = registry
            .list()
            .iter()
            .map(|r| crate::routing::persist::AliasEntry {
                domain: r.domain.clone(),
                port: r.port,
                pid: r.pid,
                managed: r.managed,
            })
            .collect();
        crate::routing::persist::save_aliases(&live);
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

    // Reap managed routes whose owner died without unregistering (SIGKILL,
    // a closed terminal). Without this they stayed in `antra list`, counted
    // as live in `doctor`, and kept the idle shutdown above from ever firing.
    let reap_registry = Arc::clone(&registry);
    tokio::spawn(async move {
        let mut reap_interval = tokio::time::interval(REAP_INTERVAL);
        loop {
            reap_interval.tick().await;
            let registry = Arc::clone(&reap_registry);
            let reaped = tokio::task::spawn_blocking(move || {
                registry.reap_dead_owners(crate::platform::is_pid_alive)
            })
            .await
            .unwrap_or_default();
            for route in reaped {
                tracing::info!(
                    domain = %route.domain,
                    pid = ?route.pid,
                    "Removed route: its process exited without unregistering"
                );
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

/// Create the directory that will hold `sock_path` as a private `0o700`
/// directory, handing it back to the invoking user when running under `sudo`.
///
/// Unix only, and deliberately a separate function from `start_daemon`: the
/// property that matters here is what the directory looks like *before* the
/// bind, which is a filesystem fact a test can assert without standing up a
/// daemon, ports and all.
#[cfg(unix)]
pub(crate) fn prepare_socket_dir(sock_path: &std::path::Path) -> std::io::Result<()> {
    let Some(parent) = sock_path.parent() else {
        return Ok(());
    };
    crate::platform::ensure_private_dir(parent)
}

fn wait_for_daemon_exit(timeout: std::time::Duration) -> Option<u32> {
    let start = std::time::Instant::now();
    loop {
        let pid = crate::ipc::server::read_daemon_pid()?;
        if !crate::platform::is_pid_alive(pid) {
            return None;
        }
        if start.elapsed() >= timeout {
            return Some(pid);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
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
        if let Some(pid) = wait_for_daemon_exit(std::time::Duration::from_secs(2)) {
            anyhow::bail!(
                "Daemon (PID {pid}) seems to be running without a socket. Stop it, then retry."
            );
        }
        let _ = std::fs::remove_file(&pid_file);
        anyhow::bail!("Daemon is not running");
    }

    #[cfg(windows)]
    if !crate::ipc::client::is_daemon_running() {
        if let Some(pid) = wait_for_daemon_exit(std::time::Duration::from_secs(2)) {
            anyhow::bail!(
                "Daemon (PID {pid}) seems to be running without IPC. Stop it, then retry."
            );
        }
        let _ = std::fs::remove_file(&pid_file);
        anyhow::bail!("Daemon is not running");
    }

    // Send shutdown command
    match crate::ipc::client::send_command_sync(crate::ipc::protocol::IpcPayload::Shutdown) {
        Ok(_) => {
            if let Some(pid) = wait_for_daemon_exit(std::time::Duration::from_secs(2)) {
                anyhow::bail!("Daemon (PID {pid}) did not stop within 2 seconds");
            }

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
            ca_fingerprint: None,
        },
    ))
}

#[cfg(all(test, unix))]
mod socket_dir_tests {
    use super::*;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn mode_of(path: &std::path::Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// The `/tmp/antra-<uid>/<hash>/` shape `socket_path()` falls back to
    /// when the derived path does not fit `sun_path`.
    fn fallback_shaped_socket(root: &std::path::Path) -> std::path::PathBuf {
        root.join("antra-501").join("3f9a1c7e").join("d.sock")
    }

    /// ROADMAP C10, gap 1. A Unix socket file is created by `bind(2)` at the
    /// process umask, so it is briefly `0o755` before `start_daemon` tightens
    /// it to `0o600` — and during that window any local user can connect and
    /// issue IPC commands. The bind-time mode is not something the daemon
    /// controls, so what has to hold is the directory: `connect` needs `x` on
    /// every component of the path, so a world-readable socket inside a
    /// `0o700` directory is unreachable to everyone else. Both levels of the
    /// chain are asserted, because one unsearchable ancestor is enough to
    /// reopen the whole path.
    #[test]
    fn socket_dir_is_unsearchable_by_others_before_the_bind() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = fallback_shaped_socket(tmp.path());

        prepare_socket_dir(&sock).unwrap();

        let per_uid = sock.parent().unwrap().parent().unwrap();
        for dir in [per_uid, sock.parent().unwrap()] {
            assert_eq!(
                mode_of(dir) & 0o077,
                0,
                "{} stays reachable by other users at the socket's bind-time mode",
                dir.display()
            );
        }

        // The bind itself still produces an unprotected file — that is the
        // window, and it is exactly why the assertion above is about the
        // directory. Run under a permissive umask so the point is not
        // accidental: a bind created here really is 0o777.
        let previous = unsafe { libc::umask(0) };
        let listener = std::os::unix::net::UnixListener::bind(&sock);
        unsafe { libc::umask(previous) };
        let _listener = listener.unwrap();
        assert_ne!(
            mode_of(&sock) & 0o077,
            0,
            "precondition: bind() creates the socket at the umask, not 0o600"
        );
    }

    /// The tightening must not lock the CLI out of its own daemon. Under
    /// `sudo antra proxy start` the directory is created by root and handed to
    /// the invoking user; the unprivileged CLI then has to be able to traverse
    /// it and connect. This asserts the same-uid half of that (the half CI can
    /// run): the prepared directory grants search to its owner and a client in
    /// that uid reaches the socket through it.
    #[test]
    fn prepared_socket_dir_keeps_the_owning_cli_reachable() {
        let tmp = tempfile::tempdir().unwrap();
        let sock = fallback_shaped_socket(tmp.path());

        prepare_socket_dir(&sock).unwrap();
        let _listener = std::os::unix::net::UnixListener::bind(&sock).unwrap();

        let parent = sock.parent().unwrap();
        assert_eq!(
            std::fs::metadata(parent).unwrap().uid(),
            unsafe { libc::geteuid() },
            "the prepared directory must belong to the user who starts the daemon"
        );
        assert_eq!(
            mode_of(parent) & 0o700,
            0o700,
            "the owner must keep rwx, or the CLI cannot reach the socket"
        );
        std::os::unix::net::UnixStream::connect(&sock)
            .expect("the owning CLI must reach the socket through a 0o700 directory");
    }
}
