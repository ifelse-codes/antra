use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::{IpcPayload, RegisterRouteRequest};
use crate::resolver::util::select_resolver;

pub fn execute(domain: &str, port: u16) -> Result<()> {
    println!("{}", "ANTRA ALIAS".bold());
    println!();

    // Check daemon is running
    if !is_daemon_running() {
        println!(
            "  {} {}",
            "⚠".yellow().bold(),
            "Daemon not running".yellow()
        );
        println!("    {}", "Run `antra proxy start` first".dimmed());
        return Ok(());
    }

    // Resolve the domain (add to hosts if needed)
    let resolver = select_resolver(domain)?;
    resolver.register(domain)?;
    println!(
        "  {} {}",
        "✓".green().bold(),
        format!("Domain resolved: {domain}").green()
    );

    // Register route via IPC
    let resp = send_command_sync(IpcPayload::RegisterRoute(RegisterRouteRequest {
        domain: domain.to_string(),
        port,
        pid: None,
    }))?;

    match resp.payload {
        IpcPayload::Ok(ok) => {
            println!("  {} {}", "✓".green().bold(), ok.message.green());
        }
        IpcPayload::Error(err) => {
            println!("  {} {}", "✗".red().bold(), err.message.red());
            return Ok(());
        }
        _ => {
            println!("  {} {}", "✗".red().bold(), "Unexpected response".red());
            return Ok(());
        }
    }

    println!();
    println!("  → {}", format!("https://{domain}").underline());
    println!();
    Ok(())
}
