use anyhow::Result;
use colored::Colorize;

use super::ProxyCommands;
use crate::daemon::server::{daemon_status, start_daemon, stop_daemon, DaemonConfig};

/// Returns the path to the daemon log file.
fn daemon_log_path() -> std::path::PathBuf {
    let dir = dirs::data_local_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("antra");
    std::fs::create_dir_all(&dir).ok();
    dir.join("daemon.log")
}

/// Check if a PID is alive on Unix.
#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(not(unix))]
fn is_pid_alive(_pid: u32) -> bool {
    // On non-Unix, fall back to socket check only
    true
}

pub fn execute(command: ProxyCommands) -> Result<()> {
    match command {
        ProxyCommands::Start {
            port,
            http_port,
            routes,
        } => {
            println!("ANTRA");
            println!();

            let config = DaemonConfig {
                https_port: port,
                http_port,
                idle_timeout: std::time::Duration::from_secs(600),
            };

            // Check if we're already in daemon mode
            if std::env::var("ANTRA_DAEMON").is_ok() {
                // Already daemonized, run directly
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(start_daemon(config))?;
            } else {
                // Spawn a separate process for the daemon
                println!("  Starting daemon...");

                let exe = std::env::current_exe()?;
                let log_path = daemon_log_path();
                let log_file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)?;

                let mut cmd = std::process::Command::new(exe);
                cmd.arg("proxy")
                    .arg("start")
                    .arg("--port")
                    .arg(port.to_string())
                    .arg("--http-port")
                    .arg(http_port.to_string())
                    .env("ANTRA_DAEMON", "1")
                    .stdout(std::process::Stdio::from(log_file.try_clone()?))
                    .stderr(std::process::Stdio::from(log_file))
                    .stdin(std::process::Stdio::null());

                // Add routes
                for route in &routes {
                    cmd.arg("--route").arg(route);
                }

                let child = cmd.spawn()?;
                let child_pid = child.id();

                // Health check loop: wait for daemon to be ready, then verify PID is alive
                let mut daemon_ready = false;
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if crate::ipc::client::is_daemon_running() {
                        daemon_ready = true;
                        break;
                    }
                }

                if !daemon_ready {
                    // Daemon never reported ready — check if process died
                    std::thread::sleep(std::time::Duration::from_millis(200));
                    if !is_pid_alive(child_pid) {
                        let log_contents = std::fs::read_to_string(&log_path)
                            .unwrap_or_default();
                        let tail = log_contents
                            .lines()
                            .rev()
                            .take(10)
                            .collect::<Vec<_>>()
                            .join("\n");
                        anyhow::bail!(
                            "Daemon process (PID: {child_pid}) crashed on startup.\n\
                             Check logs: {}\n\
                             Last output:\n{tail}",
                            log_path.display()
                        );
                    }
                    anyhow::bail!(
                        "Daemon failed to start within timeout. Check logs: {}",
                        log_path.display()
                    );
                }

                // Daemon is listening — now verify PID is actually alive
                if !is_pid_alive(child_pid) {
                    let log_contents = std::fs::read_to_string(&log_path)
                        .unwrap_or_default();
                    let tail = log_contents
                        .lines()
                        .rev()
                        .take(10)
                        .collect::<Vec<_>>()
                        .join("\n");
                    anyhow::bail!(
                        "Daemon process (PID: {child_pid}) died after socket creation.\n\
                         Check logs: {}\n\
                         Last output:\n{tail}",
                        log_path.display()
                    );
                }

                // Query actual startup status
                match crate::ipc::client::get_startup_status() {
                    Ok(status) => {
                        println!("  {} Daemon started (PID: {child_pid})", "✓".green().bold());
                        println!();
                        if status.https_ok {
                            println!(
                                "  {} HTTPS proxy on 127.0.0.1:{}",
                                "✓".green().bold(),
                                status.https_port
                            );
                        } else {
                            println!(
                                "  {} HTTPS proxy on port {}: {}",
                                "⚠".yellow().bold(),
                                status.https_port,
                                status.https_error.unwrap_or_default()
                            );
                            if status.https_port == 8443 {
                                println!(
                                    "    {}",
                                    "Tip: Run 'sudo antra proxy start' to use port 443".dimmed()
                                );
                            }
                        }
                        if status.http_ok {
                            println!(
                                "  {} HTTP→HTTPS redirect on 127.0.0.1:{}",
                                "✓".green().bold(),
                                status.http_port
                            );
                        } else {
                            println!(
                                "  {} HTTP redirect on port {}: {}",
                                "⚠".yellow().bold(),
                                status.http_port,
                                status.http_error.unwrap_or_default()
                            );
                            if status.http_port == 8080 {
                                println!(
                                    "    {}",
                                    "Tip: Run 'sudo antra proxy start' to use port 80".dimmed()
                                );
                            }
                        }
                    }
                    Err(_) => {
                        println!("  {} Daemon started (PID: {child_pid})", "✓".green().bold());
                    }
                }
                println!();

                if !routes.is_empty() {
                    println!("  Registered routes:");
                    for route_str in &routes {
                        let (domain, route_port) = parse_route(route_str)?;
                        match crate::ipc::client::send_command_ok(
                            crate::ipc::protocol::IpcPayload::RegisterRoute(
                                crate::ipc::protocol::RegisterRouteRequest {
                                    domain: domain.clone(),
                                    port: route_port,
                                    pid: None,
                                },
                            ),
                        ) {
                            Ok(msg) => println!("  {} {msg}", "✓".green().bold()),
                            Err(e) => {
                                println!("  {} Failed to register {domain}: {e}", "✗".red().bold())
                            }
                        }
                    }
                } else {
                    println!("  (No routes registered. Use --route domain:port)");
                }

                println!();
                println!(
                    "  {} Log file: {}",
                    "ℹ".cyan(),
                    log_path.display()
                );
                println!();
                return Ok(());
            }

            Ok(())
        }
        ProxyCommands::Stop => {
            println!("Stopping Antra daemon...");
            match stop_daemon() {
                Ok(()) => {
                    println!("  ✓ Daemon stopped");
                }
                Err(e) => {
                    println!("  ✗ {e}");
                }
            }
            Ok(())
        }
        ProxyCommands::Status => {
            match daemon_status() {
                Ok(status) => {
                    println!("ANTRA");
                    println!();
                    println!("  {status}");
                }
                Err(e) => {
                    println!("ANTRA");
                    println!();
                    println!("  Daemon is not running");
                    println!("  Start it with: antra proxy start");
                    println!();
                    println!("  Error: {e}");
                }
            }
            Ok(())
        }
    }
}

fn parse_route(s: &str) -> Result<(String, u16)> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid route format '{s}'. Expected domain:port");
    }
    let domain = parts[0].to_string();
    let port: u16 = parts[1]
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid port in route '{s}'"))?;
    Ok((domain, port))
}
