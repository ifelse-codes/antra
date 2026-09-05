use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;
use crate::resolver::hosts;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum HostsCommands {
    /// Sync .localhost domains to /etc/hosts for Safari compatibility
    Sync,
    /// Remove all Antra-managed entries from /etc/hosts
    Clean,
}

pub fn execute(command: HostsCommands) -> Result<()> {
    match command {
        HostsCommands::Sync => sync_hosts(),
        HostsCommands::Clean => clean_hosts(),
    }
}

fn sync_hosts() -> Result<()> {
    println!("{}", "ANTRA HOSTS SYNC".bold());
    println!();

    // Check if daemon is running
    if !is_daemon_running() {
        println!(
            "  {} {}",
            "⚠".yellow().bold(),
            "Daemon not running".yellow()
        );
        println!("    Start it with: antra proxy start");
        return Ok(());
    }

    // Get all routes from daemon
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
        println!(
            "  {} {}",
            "✓".green().bold(),
            "No routes to sync".green()
        );
        return Ok(());
    }

    // Read current hosts file
    let hosts_path = hosts::hosts_path();
    let content = hosts::read_hosts(&hosts_path)?;
    let mut content = hosts::ensure_managed_block(&content);

    let mut synced_count = 0;

    for route in &routes {
        // Only sync .localhost domains
        if route.domain.ends_with(".localhost") {
            let (new_content, added) = hosts::add_to_managed_block(&content, &route.domain);
            content = new_content;
            if added {
                println!(
                    "  {} {}",
                    "+".green().bold(),
                    format!("127.0.0.1 {}", route.domain).green()
                );
                synced_count += 1;
            } else {
                println!(
                    "  {} {}",
                    "→".dimmed(),
                    format!("{} (already present)", route.domain).dimmed()
                );
            }
        }
    }

    if synced_count > 0 {
        // Write updated hosts file
        hosts::write_hosts_atomic(&hosts_path, &content)?;
        println!();
        println!(
            "  {} Synced {} domain(s) to {}",
            "✓".green().bold(),
            synced_count,
            hosts_path.display()
        );
        println!(
            "  {}",
            "Safari should now resolve .localhost domains".dimmed()
        );
    } else {
        println!();
        println!(
            "  {} {}",
            "✓".green().bold(),
            "All .localhost domains already in hosts file".green()
        );
    }

    println!();
    Ok(())
}

fn clean_hosts() -> Result<()> {
    println!("{}", "ANTRA HOSTS CLEAN".bold());
    println!();

    // Read current hosts file
    let hosts_path = hosts::hosts_path();
    let content = hosts::read_hosts(&hosts_path)?;

    let block = hosts::extract_managed_block(&content);
    let entries: Vec<&str> = block
        .lines()
        .filter(|line| line.starts_with("127.0.0.1"))
        .collect();

    if entries.is_empty() {
        println!(
            "  {} {}",
            "✓".green().bold(),
            "No Antra-managed entries found".green()
        );
        return Ok(());
    }

    println!("  Found {} managed entr(y/ies):", entries.len());
    for entry in &entries {
        println!("  {} {}", "→".dimmed(), entry.dimmed());
    }
    println!();

    // Clear the managed block
    let content = hosts::replace_managed_block(&content, "");
    hosts::write_hosts_atomic(&hosts_path, &content)?;

    println!(
        "  {} Cleaned {} entr(y/ies) from {}",
        "✓".green().bold(),
        entries.len(),
        hosts_path.display()
    );
    println!();

    Ok(())
}
