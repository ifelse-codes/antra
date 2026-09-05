use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::{IpcPayload, UnregisterRouteRequest};

pub fn execute() -> Result<()> {
    println!("{}", "ANTRA PRUNE".bold());
    println!();

    // Check if daemon is running
    if !is_daemon_running() {
        println!(
            "  {} {}",
            "⚠".yellow().bold(),
            "Daemon not running".yellow()
        );
        println!("    {}", "Nothing to prune".dimmed());
        return Ok(());
    }

    // List all routes
    let resp = send_command_sync(IpcPayload::ListRoutes)?;
    let routes = match resp.payload {
        IpcPayload::RoutesList(list) => list.routes,
        _ => {
            println!(
                "  {} {}",
                "✗".red().bold(),
                "Unexpected response from daemon".red()
            );
            return Ok(());
        }
    };

    if routes.is_empty() {
        println!("  {} {}", "✓".green().bold(), "No routes to prune".green());
        return Ok(());
    }

    println!("  Found {} route(s)", routes.len());
    println!();

    let mut pruned_count = 0;
    let mut alive_count = 0;

    for route in &routes {
        let pid = match route.pid {
            Some(pid) => pid,
            None => {
                // No PID recorded — can't check if alive, skip
                println!("  {} {} (no PID recorded)", "→".cyan(), route.domain);
                alive_count += 1;
                continue;
            }
        };

        if is_pid_alive(pid) {
            println!(
                "  {} {} (PID {} alive)",
                "✓".green(),
                route.domain.green(),
                pid
            );
            alive_count += 1;
        } else {
            println!(
                "  {} {} (PID {} dead) — pruning",
                "✗".red(),
                route.domain.red(),
                pid
            );

            // Unregister the dead route
            match send_command_sync(IpcPayload::UnregisterRoute(UnregisterRouteRequest {
                domain: route.domain.clone(),
            })) {
                Ok(_) => {
                    println!("    {} Removed", "✓".green());
                    pruned_count += 1;
                }
                Err(e) => {
                    println!("    {} Failed to remove: {}", "✗".red(), e);
                }
            }
        }
    }

    println!();
    if pruned_count > 0 {
        println!(
            "  {} Pruned {} dead route(s), {} alive",
            "✓".green().bold(),
            pruned_count.to_string().yellow().bold(),
            alive_count.to_string().green()
        );
    } else {
        println!(
            "  {} All {} route(s) alive",
            "✓".green().bold(),
            alive_count
        );
    }
    println!();

    Ok(())
}

/// Check if a PID is alive using signal 0.
#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    kill(Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(not(unix))]
fn is_pid_alive(_pid: u32) -> bool {
    // Fallback: assume alive on non-Unix
    true
}
