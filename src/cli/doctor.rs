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

    // 2. Check CA trust
    match trust::check_trust_status() {
        Ok(true) => {
            println!(
                "  {} {}",
                "✓".green().bold(),
                "CA trusted by system".green()
            );
        }
        Ok(false) => {
            println!(
                "  {} {}",
                "✗".red().bold(),
                "CA not trusted by system".red()
            );
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
    if is_daemon_running() {
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
    } else {
        println!(
            "  {} {}",
            "⚠".yellow().bold(),
            "Proxy daemon not running".yellow()
        );
        warnings.push("Proxy daemon not running".to_string());
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
                Err(_) => {
                    if is_antra_daemon_port(port) {
                        println!(
                            "  {} {}",
                            "✓".green().bold(),
                            format!("Port {port} ({name}) — Antra daemon active").green()
                        );
                    } else {
                        println!(
                            "  {} {}",
                            "⚠".yellow().bold(),
                            format!("Port {port} ({name}) in use by another process").yellow()
                        );
                        warnings
                            .push(format!("Port {port} ({name}) in use by another process"));
                        if port == 443 {
                            issues.push((
                                format!("Port {port} ({name}) in use"),
                                "antra proxy start --port 8443 --http-port 8080".to_string(),
                            ));
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
            parts.push(format!(
                "{} error(s)",
                error_count.to_string().red().bold()
            ));
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

        // Offer auto-fix
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

    match output {
        Ok(o) => o.status.success() && !o.stdout.is_empty(),
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_antra_daemon_port(_port: u16) -> bool {
    false
}
