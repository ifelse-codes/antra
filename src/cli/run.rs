use anyhow::Result;
use clap::Args;
use colored::Colorize;
use tokio::process::Command;

use crate::config::global;
use crate::ipc::client::is_daemon_running;
use crate::resolver::util::select_resolver;
use crate::util::output;
use crate::util::port::{
    detect_port_from_command, find_free_port, find_free_port_in_range, inject_port_flag,
    is_port_available,
};
use crate::util::port_watcher;

#[derive(Args)]
pub struct RunArgs {
    /// Domain to proxy (e.g., myapp.localhost)
    #[arg(long)]
    pub domain: String,

    /// Port the application listens on (auto-detected if omitted)
    #[arg(long)]
    pub port: Option<u16>,

    /// Custom TLD (e.g., dev.example.com for myapp.dev.example.com)
    #[arg(long)]
    pub tld: Option<String>,

    /// Allow custom (non-.localhost, non-.test) domains
    #[arg(long)]
    pub allow_custom_domain: bool,

    /// Skip the trust CA prompt on first run
    #[arg(long)]
    pub no_trust_prompt: bool,

    /// Skip prompts and auto-install CA
    #[arg(short, long)]
    pub yes: bool,

    /// Kill existing process and take over the route
    #[arg(long)]
    pub force: bool,

    /// Command to run
    #[arg(trailing_var_arg = true, required = true)]
    pub command: Vec<String>,
}

pub fn execute(args: RunArgs) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { run_inner(args).await })
}

/// Auto-install the CA on first run (no prompt).
/// Only runs once per install. Falls back gracefully if sudo is unavailable.
async fn maybe_prompt_trust(no_trust_prompt: bool, _yes: bool) {
    // Skip if user explicitly opted out
    if no_trust_prompt {
        return;
    }

    // Never block on OS auth dialogs without a terminal (background jobs,
    // pipes, CI). The `security` prompt would hang forever with no TTY.
    #[cfg(unix)]
    let interactive = unsafe { libc::isatty(libc::STDIN_FILENO) != 0 };
    #[cfg(not(unix))]
    let interactive = true;
    // macOS-first hint: user-level trust needs no sudo.
    #[cfg(target_os = "macos")]
    let trust_hint = "antra trust --user-level";
    #[cfg(not(target_os = "macos"))]
    let trust_hint = "antra trust";
    if !interactive {
        println!();
        println!(
            "  {} Non-interactive session — skipping automatic CA install.",
            "ℹ".cyan()
        );
        println!("  Run {} to enable warning-free HTTPS", trust_hint.bold());
        println!();
        return;
    }

    // Skip if already prompted before
    if global::was_trust_prompted() {
        return;
    }

    // Check if CA is already trusted (system store OR macOS login keychain —
    // user-level trust alone gives warning-free HTTPS, no reinstall needed)
    if crate::trust::is_trusted_for_https() {
        let _ = global::mark_trust_prompted();
        return;
    }

    // CA not trusted — auto-install it
    println!();
    println!("  {} Setting up HTTPS (one-time)...", "▸".cyan());
    match crate::trust::install_ca_noninteractive() {
        Ok(()) => {
            println!("  {} CA installed — HTTPS ready", "✓".green().bold());
            let _ = global::mark_trust_prompted();
        }
        Err(e) => {
            println!("  {} Auto-trust failed: {e}", "⚠".yellow());
            println!(
                "  Run {} to install the CA, then re-run your command",
                trust_hint.bold()
            );
        }
    }
    println!();
}

/// Ensure the daemon is running, starting it if necessary
async fn ensure_daemon() -> Result<()> {
    if is_daemon_running() {
        return Ok(());
    }

    output::print_warning("Daemon not running, starting it...");

    // Start the daemon as a separate process
    let exe = std::env::current_exe()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("proxy")
        .arg("start")
        .env("ANTRA_DAEMON", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());

    let child = cmd.spawn()?;
    let _child_pid = child.id();

    // Wait for daemon to start
    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        if is_daemon_running() {
            output::print_success("Daemon started");
            return Ok(());
        }
    }

    anyhow::bail!("Daemon failed to start within timeout");
}

async fn run_inner(args: RunArgs) -> Result<()> {
    output::print_header();

    // 0. Auto-trust prompt (first run only)
    maybe_prompt_trust(args.no_trust_prompt, args.yes).await;

    // 0b. Handle custom TLD
    let domain = if let Some(tld) = &args.tld {
        // Build domain with custom TLD: appname.tld
        let app_name = args.domain.split('.').next().unwrap_or(&args.domain);
        format!("{app_name}.{tld}")
    } else {
        args.domain.clone()
    };

    // 1. Determine port
    let port = match args.port {
        Some(p) => {
            // User specified a port — honor it verbatim. Only remap on a real
            // conflict, and say so loudly (silent remaps route traffic nowhere).
            if is_port_available(p) {
                output::print_success(&format!("Using port {p}"));
                p
            } else if detect_port_from_command(&args.command) == Some(p) {
                // The child's own args pin it to the busy port (e.g.
                // `python3 -m http.server 18090`), so spawning it would only
                // dump a raw `Address already in use` traceback. Fail fast
                // with the actionable message instead — before registering
                // anything, so there is nothing to clean up.
                output::print_error(&format!("Port {p} is already in use."));
                output::print_warning(&format!(
                    "Your command looks pinned to port {p} (`{}`), so it cannot start there.",
                    args.command.join(" ")
                ));
                output::print_warning(
                    "Stop the process on that port (see `antra list`), or pass a free --port.",
                );
                return Err(anyhow::anyhow!("Port {p} is already in use"));
            } else {
                let alt = find_free_port()?;
                output::print_warning(&format!(
                    "Port {p} is already in use. Assigned port {alt} instead."
                ));
                output::print_warning(
                    "Tip: make sure your server listens on $PORT, or pass a free --port.",
                );
                alt
            }
        }
        None => {
            // Try to detect port from command arguments
            if let Some(detected) = detect_port_from_command(&args.command) {
                tracing::debug!(port = detected, "Auto-detected port from command args");
                output::print_success(&format!("Detected port {detected} from command"));
                detected
            } else {
                tracing::debug!(command = ?args.command, "Could not auto-detect port from command");
                let detected = find_free_port_in_range()?;
                output::print_warning(&format!(
                    "Could not detect port from command. Auto-assigned port {detected}."
                ));
                output::print_warning(
                    "Tip: Use --port to specify the port your server listens on.",
                );
                detected
            }
        }
    };

    // 1b. Check for an existing route on this domain. Never silently
    // clobber it (parity with `alias`, which warns): record the previous
    // mapping so it can be warned about now and restored if the new
    // backend fails. `--force` has its own louder message below.
    let mut replaced_port: Option<u16> = None;
    if let Ok(resp) =
        crate::ipc::client::send_command(crate::ipc::protocol::IpcPayload::ListRoutes).await
    {
        if let crate::ipc::protocol::IpcPayload::RoutesList(list) = resp.payload {
            if let Some(existing) = list.routes.iter().find(|r| r.domain == domain) {
                replaced_port = Some(existing.port);
                if !args.force && existing.port != port {
                    output::print_warning(&format!(
                        "Domain {domain} is already routed to port {} — replacing with port {port}",
                        existing.port
                    ));
                }
            }
        }
    }

    // 1c. Handle --force: kill existing route if present
    if args.force {
        if let Ok(resp) =
            crate::ipc::client::send_command(crate::ipc::protocol::IpcPayload::ListRoutes).await
        {
            if let crate::ipc::protocol::IpcPayload::RoutesList(list) = resp.payload {
                if let Some(existing) = list.routes.iter().find(|r| r.domain == domain) {
                    output::print_warning(&format!(
                        "Domain {} already in use (port {}, PID {:?})",
                        existing.domain, existing.port, existing.pid
                    ));

                    // Kill the existing process if PID is known
                    if let Some(pid) = existing.pid {
                        output::print_warning(&format!("Killing process {pid}..."));
                        kill_process(pid);
                    }

                    // Unregister the old route
                    let _ = crate::ipc::client::send_command(
                        crate::ipc::protocol::IpcPayload::UnregisterRoute(
                            crate::ipc::protocol::UnregisterRouteRequest {
                                domain: domain.clone(),
                            },
                        ),
                    )
                    .await;

                    output::print_success("Old route removed");
                }
            }
        }
    }

    // 2. Resolve domain to 127.0.0.1 (hosts file or no-op).
    // --allow-custom-domain bypasses the known-public-domain blocklist.
    let resolver: Box<dyn crate::resolver::traits::DomainResolver> =
        if args.allow_custom_domain && crate::resolver::util::is_custom_domain(&domain) {
            Box::new(crate::resolver::custom::CustomResolver::new().with_allow_public(true))
        } else {
            select_resolver(&domain)?
        };
    resolver.register(&domain)?;
    output::print_success(&format!("Domain resolved: {}", domain));

    // 3. Ensure daemon is running
    ensure_daemon().await?;

    // 4. Register route via IPC
    match crate::ipc::client::send_command(crate::ipc::protocol::IpcPayload::RegisterRoute(
        crate::ipc::protocol::RegisterRouteRequest {
            domain: domain.clone(),
            port,
            pid: None,
        },
    ))
    .await
    {
        Ok(msg) => match msg.payload {
            crate::ipc::protocol::IpcPayload::Ok(ok) => {
                output::print_success(&ok.message);
            }
            crate::ipc::protocol::IpcPayload::Error(err) => {
                output::print_error(&err.message);
                return Err(anyhow::anyhow!("{}", err.message));
            }
            other => {
                output::print_error(&format!("Unexpected response: {other:?}"));
                return Err(anyhow::anyhow!("Unexpected IPC response"));
            }
        },
        Err(e) => {
            output::print_error(&format!("Failed to register route: {e}"));
            return Err(e);
        }
    }

    println!();
    // Print the actual URL the user should visit
    if let Ok(status) = crate::ipc::client::get_startup_status_async().await {
        if status.https_port != 443 {
            println!(
                "  {} Note: HTTPS on port {} (port 443 unavailable — needs sudo or is in use)",
                "ℹ".cyan(),
                status.https_port
            );
            println!(
                "  {} To use port 443: {}",
                "ℹ".cyan(),
                "sudo antra proxy start".bold()
            );
            // Build clean URL — don't double-append .localhost
            let host = if domain.ends_with(".localhost") {
                domain.clone()
            } else {
                format!("{}.localhost", domain)
            };
            println!("  → https://{}:{}", host, status.https_port);
        } else {
            println!("  → https://{}", domain);
        }
    } else {
        println!("  → https://{}", domain);
    }
    println!();

    // 5. Spawn child process
    let program = args.command.first().unwrap_or(&"".to_string()).clone();

    // Inject --port flag for frameworks that ignore PORT env var
    let final_args = inject_port_flag(&args.command, port);
    let final_program = final_args.first().unwrap_or(&program).clone();
    let final_child_args: Vec<String> = final_args[1..].to_vec();

    // Build environment with CA cert path for Node.js TLS trust
    let mut cmd = Command::new(&final_program);
    cmd.args(&final_child_args)
        .env("PORT", port.to_string())
        .env("HOST", "127.0.0.1")
        .env("ANTRA_DOMAIN", &domain)
        .env("ANTRA_URL", format!("https://{}", domain))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit());

    // Inject NODE_EXTRA_CA_CERTS if CA exists
    if let Ok(store) = crate::certs::store::CertStore::new() {
        let ca_path = store.config_dir.join("ca.pem");
        if ca_path.exists() {
            cmd.env("NODE_EXTRA_CA_CERTS", ca_path.to_string_lossy().to_string());
        }
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            // The backend never started — don't leave a dangling route
            // behind pointing at a port nothing listens on. If this run
            // replaced an existing mapping (and --force didn't kill its
            // owner), put the previous route back instead of deleting it.
            if !args.force {
                if let Some(prev) = replaced_port {
                    restore_previous_route(&domain, prev).await;
                    return Err(anyhow::anyhow!("Failed to spawn '{}': {e}", final_program));
                }
            }
            let _ = crate::ipc::client::send_command(
                crate::ipc::protocol::IpcPayload::UnregisterRoute(
                    crate::ipc::protocol::UnregisterRouteRequest {
                        domain: domain.clone(),
                    },
                ),
            )
            .await;
            let _ = resolver.unregister(&domain);
            output::print_warning(&format!("Route removed: {domain}"));
            return Err(anyhow::anyhow!("Failed to spawn '{}': {e}", final_program));
        }
    };

    // Capture stdout for port watching
    let child_stdout = child.stdout.take();

    output::print_success(&format!("Started: {}", final_args.join(" ")));
    println!();

    // Start port watcher if stdout is captured
    if let Some(stdout) = child_stdout {
        port_watcher::watch_port_changes(stdout, domain.clone(), port);
    }

    // 6. Wait for child or signal
    let child_pid = child.id();

    // Set up signal handler
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(unix)]
    let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    // Wait for child to exit or signal
    let exit_code = tokio::select! {
        status = child.wait() => {
            match status {
                Ok(s) => s.code().unwrap_or(1),
                Err(e) => {
                    eprintln!("Error waiting for child: {e}");
                    1
                }
            }
        }
        _ = async {
            #[cfg(unix)]
            {
                tokio::select! {
                    _ = sigterm.recv() => {},
                    _ = sigint.recv() => {},
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c().await.ok();
            }
        } => {
            // Signal received — forward to child
            output::print_warning("Signal received, shutting down...");

            #[cfg(unix)]
            {
                use nix::sys::signal::{killpg, Signal};
                use nix::unistd::getpgid;

                if let Some(pid) = child_pid {
                    if let Ok(pgid) = getpgid(Some(nix::unistd::Pid::from_raw(pid as i32))) {
                        let _ = killpg(pgid, Signal::SIGTERM);
                    }
                }
            }

            #[cfg(windows)]
            {
                // On Windows, attempt to kill the process tree via taskkill
                if let Some(pid) = child_pid {
                    let _ = std::process::Command::new("taskkill")
                        .args(["/F", "/T", "/PID", &pid.to_string()])
                        .output();
                }
            }

            // Wait briefly for child to exit
            match tokio::time::timeout(
                std::time::Duration::from_secs(5),
                child.wait()
            ).await {
                Ok(Ok(s)) => s.code().unwrap_or(1),
                _ => {
                    // Force kill
                    let _ = child.kill().await;
                    130
                }
            }
        }
    };

    // 7. Report backend failures helpfully. The child's own stderr is
    // inherited above (raw tracebacks like `OSError: Address already in
    // use` are cryptic on their own), so add the actionable hint here.
    if exit_code != 0 {
        output::print_warning(&format!("Command exited with code {exit_code}"));
        output::print_warning(
            "Tip: if the error above is 'Address already in use', the port is occupied — see `antra list` or pass a free --port.",
        );
    }

    // 8. Cleanup - unregister route via IPC. If this run replaced an
    // existing mapping (and --force didn't kill its owner), restore the
    // previous route instead of leaving the domain routeless.
    if !args.force {
        if let Some(prev) = replaced_port {
            restore_previous_route(&domain, prev).await;
            println!();
            std::process::exit(exit_code);
        }
    }
    let _ = crate::ipc::client::send_command(crate::ipc::protocol::IpcPayload::UnregisterRoute(
        crate::ipc::protocol::UnregisterRouteRequest {
            domain: domain.clone(),
        },
    ))
    .await;
    resolver.unregister(&domain)?;
    output::print_warning(&format!("Route removed: {domain}"));

    println!();
    std::process::exit(exit_code);
}

/// Re-register a previously replaced route (best-effort).
///
/// When `run` overwrites an existing domain mapping and the new backend
/// fails, the domain must not be left with no route: put the previous
/// mapping back instead of deleting the user's working route.
async fn restore_previous_route(domain: &str, port: u16) {
    let _ = crate::ipc::client::send_command(crate::ipc::protocol::IpcPayload::RegisterRoute(
        crate::ipc::protocol::RegisterRouteRequest {
            domain: domain.to_string(),
            port,
            pid: None,
        },
    ))
    .await;
    output::print_warning(&format!(
        "Restored previous route: {domain} → 127.0.0.1:{port}"
    ));
}

/// Kill a process by PID.
#[cfg(unix)]
fn kill_process(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;

    // Try SIGTERM first for graceful shutdown
    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);

    // Wait a bit, then force kill if still alive
    std::thread::sleep(std::time::Duration::from_millis(500));
    if is_pid_alive(pid) {
        let _ = kill(Pid::from_raw(pid as i32), Signal::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_process(_pid: u32) {
    // On Windows, would need taskkill
    // For now, no-op
}

/// Check if a PID is alive.
#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(not(unix))]
#[allow(dead_code)]
fn is_pid_alive(_pid: u32) -> bool {
    true
}
