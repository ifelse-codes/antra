use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;

pub fn execute() -> Result<()> {
    println!("{}", "ACTIVE ROUTES".bold());
    println!();

    if !is_daemon_running() {
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

                for route in &list.routes {
                    let pid_str = match route.pid {
                        Some(pid) => pid.to_string(),
                        None => "—".to_string(),
                    };
                    let uptime = format_duration(route.created_at_secs);
                    println!(
                        "  {:<40} {:<10} {:<10} {}",
                        route.domain.green().bold(),
                        route.port.to_string().cyan(),
                        pid_str,
                        uptime.dimmed(),
                    );
                }
                println!();
                println!("  {} route(s)", list.routes.len().to_string().cyan());
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

fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}
