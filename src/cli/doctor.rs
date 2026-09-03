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
    if issues.is_empty() {
        println!("  {}", "Everything looks good!".green().bold());
    } else {
        println!(
            "  {} issue(s) found:",
            issues.len().to_string().red().bold()
        );
        println!();
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
                return check_port_holder(pid, port);
            }
        }
    }
    false
}

#[cfg(not(unix))]
fn is_antra_daemon_port(_port: u16) -> bool {
    false
}

/// Check if a specific PID holds a port.
#[cfg(unix)]
fn check_port_holder(pid: u32, port: u16) -> bool {
    // Use lsof to check if the PID holds the port
    let output = std::process::Command::new("lsof")
        .args([
            "-p",
            &pid.to_string(),
            "-i",
            &format!(":{port}"),
            "-n",
            "-P",
        ])
        .output();

    match output {
        Ok(o) => o.status.success() && !o.stdout.is_empty(),
        Err(_) => false,
    }
}
