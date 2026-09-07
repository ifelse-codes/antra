use anyhow::Result;
use colored::Colorize;

use crate::certs::store::CertStore;
use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;
use crate::trust;

pub fn execute() -> Result<()> {
    println!("{}", "ANTRA DOCTOR".bold());
    println!();
    println!("  Checking your Antra setup...");
    println!();

    let mut issues: Vec<(String, String)> = Vec::new(); // (issue, fix_command)
    let mut warnings: Vec<String> = Vec::new(); // warning messages

    // 1. Check CA generation
    match CertStore::new() {
        Ok(store) => {
            if store.ca_exists() {
                println!("  {} {}", "✓".green().bold(), "Root CA generated".green());
            } else {
                println!("  {} {}", "✗".red().bold(), "Root CA not generated".red());
                issues.push((
                    "Root CA not generated".to_string(),
                    "antra trust".to_string(),
                ));
            }
        }
        Err(e) => {
            println!(
                "  {} {}",
                "✗".red().bold(),
                format!("Cert store error: {e}").red()
            );
            issues.push((
                format!("Cert store error: {e}"),
                "antra clean && antra trust".to_string(),
            ));
        }
    }

    // 2. Check CA trust (system store, then macOS user login keychain)
    match trust::check_trust_status() {
        Ok(true) => {
            println!(
                "  {} {}",
                "✓".green().bold(),
                "CA trusted by system".green()
            );
        }
        Ok(false) if trust::check_user_level_trust() => {
            println!(
                "  {} {}",
                "✓".green().bold(),
                "CA trusted via login keychain (user-level, no sudo)".green()
            );
        }
        Ok(false) => {
            println!(
                "  {} {}",
                "✗".red().bold(),
                "CA not trusted by system".red()
            );
            #[cfg(target_os = "macos")]
            issues.push((
                "CA not trusted (no warning-free HTTPS)".to_string(),
                // Single executable command so `Auto-fix all issues?` works
                // with one keypress — no sudo needed on macOS.
                "antra trust --user-level".to_string(),
            ));
            #[cfg(not(target_os = "macos"))]
            issues.push((
                "CA not trusted by system".to_string(),
                "antra trust".to_string(),
            ));
        }
        Err(e) => {
            println!(
                "  {} {}",
                "?".yellow().bold(),
                format!("Could not check trust status: {e}").yellow()
            );
            warnings.push(format!("Could not check trust status: {e}"));
        }
    }

    // 3. Check daemon status
    // Each condition is reported exactly once: blocking problems go to
    // `issues` (exit 2, with a fix command), informational notes go to
    // `warnings` (exit 1). Nothing is counted as both.
    let daemon_running = is_daemon_running();
    if daemon_running {
        println!(
            "  {} {}",
            "✓".green().bold(),
            "Proxy daemon running".green()
        );

        // Get route count
        if let Ok(resp) = send_command_sync(IpcPayload::ListRoutes) {
            if let IpcPayload::RoutesList(list) = resp.payload {
                println!(
                    "    {}",
                    format!("{} active route(s)", list.routes.len()).dimmed()
                );
            }
        }

        // Get status
        if let Ok(resp) =
            send_command_sync(IpcPayload::Status(crate::ipc::protocol::StatusResponse {
                pid: 0,
                uptime_secs: 0,
                route_count: 0,
                socket_path: String::new(),
            }))
        {
            if let IpcPayload::Status(status) = resp.payload {
                println!("    {}", format!("PID: {}", status.pid).dimmed());
                println!(
                    "    {}",
                    format!("Uptime: {}s", status.uptime_secs).dimmed()
                );
            }
        }

        // Show the ports the daemon actually bound (fallbacks included),
        // so users know which URLs to visit.
        if let Ok(startup) = crate::ipc::client::get_startup_status() {
            println!(
                "    {}",
                format!(
                    "HTTPS :{} · HTTP :{}",
                    startup.https_port, startup.http_port
                )
                .dimmed()
            );
            if startup.https_port != 443 {
                println!(
                    "    {}",
                    format!(
                        "Visit https://<domain>:{} (port 443 unavailable)",
                        startup.https_port
                    )
                    .dimmed()
                );
            }
        }
    } else {
        // Counted as an error below (exit 2), so display it as one: ✗ red,
        // not ⚠ yellow. Glyphs always match their bucket (✗ = error,
        // ⚠ = warning) so the "N error(s), N warning(s)" summary is exact.
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Proxy daemon not running".red()
        );
        issues.push((
            "Proxy daemon not running".to_string(),
            "antra proxy start".to_string(),
        ));
    }

    // 4. Check ports
    {
        use std::net::TcpListener;

        let ports = [(443, "HTTPS"), (80, "HTTP redirect")];
        for (port, name) in ports {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(_) => {
                    println!(
                        "  {} {}",
                        "✓".green().bold(),
                        format!("Port {port} ({name}) available").green()
                    );
                }
                Err(e) => {
                    if is_antra_daemon_port(port) {
                        println!(
                            "  {} {}",
                            "✓".green().bold(),
                            format!("Port {port} ({name}) — Antra daemon active").green()
                        );
                    } else if e.kind() == std::io::ErrorKind::PermissionDenied {
                        // Privileged port + non-root: NOT "in use", just not permitted.
                        // Blocking when the daemon isn't up (fresh `proxy start`
                        // can't bind) → error (✗); informational once the daemon
                        // already runs on fallback ports → warning (⚠).
                        // Glyph always matches the bucket below.
                        let is_error = !daemon_running;
                        if is_error {
                            println!(
                                "  {} {}",
                                "✗".red().bold(),
                                format!("Port {port} ({name}) needs elevated privileges").red()
                            );
                        } else {
                            println!(
                                "  {} {}",
                                "⚠".yellow().bold(),
                                format!("Port {port} ({name}) needs elevated privileges").yellow()
                            );
                        }
                        if !daemon_running {
                            issues.push((
                                format!("Port {port} ({name}) needs elevated privileges"),
                                "sudo antra proxy start  OR  antra proxy start --port 8443 --http-port 8080".to_string(),
                            ));
                        } else {
                            warnings.push(format!(
                                "Port {port} ({name}) needs elevated privileges (run with sudo or use fallback ports)"
                            ));
                        }
                    } else {
                        // Same rule: a down daemon can't start at all (blocking
                        // → error ✗); a running daemon already worked around it
                        // (note it → warning ⚠). Glyph matches the bucket.
                        if !daemon_running {
                            println!(
                                "  {} {}",
                                "✗".red().bold(),
                                format!("Port {port} ({name}) in use by another process").red()
                            );
                        } else {
                            println!(
                                "  {} {}",
                                "⚠".yellow().bold(),
                                format!("Port {port} ({name}) in use by another process").yellow()
                            );
                        }
                        if !daemon_running {
                            issues.push((
                                format!("Port {port} ({name}) in use"),
                                "antra proxy start --port 8443 --http-port 8080".to_string(),
                            ));
                        } else {
                            warnings
                                .push(format!("Port {port} ({name}) in use by another process"));
                        }
                    }
                }
            }
        }
    }

    println!();
    let error_count = issues.len();
    let warning_count = warnings.len();

    if error_count == 0 && warning_count == 0 {
        println!("  {}", "Everything looks good!".green().bold());
    } else {
        // Build summary line
        let mut parts = Vec::new();
        if error_count > 0 {
            parts.push(format!("{} error(s)", error_count.to_string().red().bold()));
        }
        if warning_count > 0 {
            parts.push(format!(
                "{} warning(s)",
                warning_count.to_string().yellow().bold()
            ));
        }
        println!("  {} found:", parts.join(", "));
        println!();

        // Print warnings first (non-blocking)
        for warning in &warnings {
            println!("  {} {}", "⚠".yellow(), warning.yellow());
        }
        if !warnings.is_empty() && !issues.is_empty() {
            println!();
        }

        // Print errors (blocking issues)
        for (issue, fix) in &issues {
            println!("  {} {}", "•".red(), issue.red());
            println!("    {} {}", "→".cyan(), fix.cyan());
        }
        println!();

        // Offer auto-fix (only if stdin is a TTY)
        #[cfg(unix)]
        let is_tty = unsafe { libc::isatty(libc::STDIN_FILENO) != 0 };
        #[cfg(not(unix))]
        let is_tty = true; // On Windows, assume TTY for simplicity
        if is_tty {
            print!("  {} ", "Auto-fix all issues? [y/N]".yellow().bold());
            use std::io::Write;
            let _ = std::io::stdout().flush();

            let mut input = String::new();
            if std::io::stdin().read_line(&mut input).is_ok() {
                let input = input.trim().to_lowercase();
                if input == "y" || input == "yes" {
                    println!();
                    auto_fix(&issues);
                } else {
                    println!();
                    println!(
                        "  {}",
                        "Run the commands above manually to fix issues.".dimmed()
                    );
                }
            }
        } else {
            println!();
            println!(
                "  {}",
                "Run the commands above manually to fix issues.".dimmed()
            );
        }
    }

    println!();

    // Exit with appropriate code: 0=clean, 1=warnings only, 2=errors
    if error_count > 0 {
        std::process::exit(2);
    } else if warning_count > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn auto_fix(issues: &[(String, String)]) {
    for (issue, fix) in issues {
        println!("  {} Fixing: {}", "→".cyan(), issue);
        println!("    {} {}", "$".dimmed(), fix.dimmed());

        // Parse and execute the fix command
        let parts: Vec<&str> = fix.split_whitespace().collect();
        if parts.is_empty() {
            println!("    {}", "Skipping: empty command".yellow());
            continue;
        }

        let result = std::process::Command::new(parts[0])
            .args(&parts[1..])
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    println!("    {} Fixed", "✓".green());
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!("    {} Failed: {}", "✗".red(), stderr.trim());
                    println!(
                        "    {}",
                        "You may need to run this manually with sudo.".dimmed()
                    );
                }
            }
            Err(e) => {
                println!("    {} Failed to execute: {}", "✗".red(), e);
            }
        }
        println!();
    }
}

/// Check if a port is held by the Antra daemon process.
#[cfg(unix)]
fn is_antra_daemon_port(port: u16) -> bool {
    let pid_path = crate::ipc::server::pid_path();
    if let Ok(pid_str) = std::fs::read_to_string(&pid_path) {
        if let Ok(pid) = pid_str.trim().parse::<u32>() {
            use nix::sys::signal::kill;
            use nix::unistd::Pid;
            if kill(Pid::from_raw(pid as i32), None).is_ok() {
                // Process is alive — check if it holds the port via /proc or lsof with timeout
                return check_port_holder_with_timeout(pid, port);
            }
        }
    }
    false
}

/// Check if a PID holds a port, with a timeout to avoid hanging.
#[cfg(unix)]
fn check_port_holder_with_timeout(pid: u32, port: u16) -> bool {
    use std::process::Command;

    let output = Command::new("lsof")
        .args([
            "-p",
            &pid.to_string(),
            "-i",
            &format!(":{port}"),
            "-n",
            "-P",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    // lsof exits 0 even with no matches on some platforms, so require an
    // actual LISTEN line for the port instead of trusting the exit code.
    // Match the port exactly: a naive `contains(":80")` also matches
    // `:8080`, which once made doctor credit the daemon with port 80
    // while it was really on the 8080 fallback.
    match output {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout);
            out.lines().any(|l| {
                l.contains("LISTEN")
                    && l.split_whitespace().any(|token| {
                        token
                            .rsplit(':')
                            .next()
                            .map(|port_str| {
                                port_str
                                    .trim_end_matches(|c: char| !c.is_ascii_digit())
                                    .parse::<u16>()
                                    .map(|p| p == port)
                                    .unwrap_or(false)
                            })
                            .unwrap_or(false)
                    })
            })
        }
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_antra_daemon_port(_port: u16) -> bool {
    false
}
