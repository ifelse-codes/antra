use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::{IpcPayload, RegisterRouteRequest};
use crate::resolver::traits::DomainResolver;

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

fn select_resolver(domain: &str) -> Result<Box<dyn DomainResolver>> {
    if domain == "localhost" || domain.ends_with(".localhost") {
        Ok(Box::new(crate::resolver::localhost::LocalhostResolver))
    } else if domain.ends_with(".test") {
        Ok(Box::new(crate::resolver::test::HostsResolver::new()))
    } else {
        Ok(Box::new(crate::resolver::custom::CustomResolver::new()))
    }
}
