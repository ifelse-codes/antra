use anyhow::Result;

use super::ProxyCommands;
use crate::daemon::server::{daemon_status, start_daemon, stop_daemon, DaemonConfig};

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
                let mut cmd = std::process::Command::new(exe);
                cmd.arg("proxy")
                    .arg("start")
                    .arg("--port")
                    .arg(port.to_string())
                    .arg("--http-port")
                    .arg(http_port.to_string())
                    .env("ANTRA_DAEMON", "1")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .stdin(std::process::Stdio::null());

                // Add routes
                for route in &routes {
                    cmd.arg("--route").arg(route);
                }

                let child = cmd.spawn()?;
                let child_pid = child.id();

                // Wait for daemon to start
                for _ in 0..20 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if crate::ipc::client::is_daemon_running() {
                        // Query actual startup status
                        match crate::ipc::client::get_startup_status() {
                            Ok(status) => {
                                println!("  ✓ Daemon started (PID: {child_pid})");
                                println!();
                                if status.https_ok {
                                    println!("  ✓ HTTPS proxy on 127.0.0.1:{}", status.https_port);
                                } else {
                                    println!(
                                        "  ⚠ HTTPS proxy on port {}: {}",
                                        status.https_port,
                                        status.https_error.unwrap_or_default()
                                    );
                                }
                                if status.http_ok {
                                    println!(
                                        "  ✓ HTTP→HTTPS redirect on 127.0.0.1:{}",
                                        status.http_port
                                    );
                                } else {
                                    println!(
                                        "  ⚠ HTTP redirect on port {}: {}",
                                        status.http_port,
                                        status.http_error.unwrap_or_default()
                                    );
                                }
                            }
                            Err(_) => {
                                println!("  ✓ Daemon started (PID: {child_pid})");
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
                                    Ok(msg) => println!("  ✓ {msg}"),
                                    Err(e) => println!("  ✗ Failed to register {domain}: {e}"),
                                }
                            }
                        } else {
                            println!("  (No routes registered. Use --route domain:port)");
                        }

                        println!();
                        return Ok(());
                    }
                }

                anyhow::bail!("Daemon failed to start within timeout");
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
