use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;

pub fn execute() -> Result<()> {
    println!("{}", "ACTIVE ROUTES".bold());
    println!();

    if !is_daemon_running() {
        #[cfg(unix)]
        if crate::platform::daemon_socket_permission_denied() {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Daemon running as root (socket permission denied)".yellow()
            );
            println!("    Use {} to manage it.", "sudo antra proxy status".cyan());
            return Ok(());
        }
        println!(
            "  {} {}",
            "⚠".yellow().bold(),
            "Daemon not running".yellow()
        );
        println!("    Run {} to start.", "antra proxy start".cyan());
        return Ok(());
    }

    let resp = send_command_sync(IpcPayload::ListRoutes)?;
    match resp.payload {
        IpcPayload::RoutesList(list) => {
            if list.routes.is_empty() {
                println!("  (No active routes)");
            } else {
                // Table header
                println!(
                    "  {:<40} {:<10} {:<10} {}",
                    "DOMAIN".dimmed(),
                    "PORT".dimmed(),
                    "PID".dimmed(),
                    "UPTIME".dimmed(),
                );
                println!("  {}", "─".repeat(75).dimmed());

                let mut exited = 0usize;
                for route in &list.routes {
                    // A daemon reaps these within seconds, but one started by
                    // an older release never does — say so rather than list
                    // a dead route as active.
                    let gone = route_owner_exited(route);
                    if gone {
                        exited += 1;
                    }
                    let pid_str = match route.pid {
                        Some(pid) => pid.to_string(),
                        None => "—".to_string(),
                    };
                    let uptime = format_duration(route.created_at_secs);
                    let status = if gone {
                        format!("  {}", "process exited".yellow())
                    } else {
                        String::new()
                    };
                    println!(
                        "  {:<40} {:<10} {:<10} {}{}",
                        route.domain.green().bold(),
                        route.port.to_string().cyan(),
                        pid_str,
                        uptime.dimmed(),
                        status,
                    );
                }
                println!();
                println!("  {} route(s)", list.routes.len().to_string().cyan());
                if exited > 0 {
                    println!(
                        "  {} {} route(s) belong to a process that has exited. Remove them: {}",
                        "⚠".yellow().bold(),
                        exited,
                        "antra prune".cyan()
                    );
                }
            }
        }
        IpcPayload::Error(err) => {
            println!("  {} {}", "✗".red().bold(), err.message.red());
        }
        _ => {
            println!("  {} {}", "✗".red().bold(), "Unexpected response".red());
        }
    }

    Ok(())
}

/// A managed route whose owner process is gone. Static routes have no owner.
pub(crate) fn route_owner_exited(route: &crate::ipc::protocol::RouteInfo) -> bool {
    route.managed
        && route
            .pid
            .is_some_and(|pid| !crate::platform::is_pid_alive(pid))
}

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
