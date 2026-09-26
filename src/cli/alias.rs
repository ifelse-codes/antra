use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::{IpcPayload, RegisterRouteRequest};
use crate::resolver::util::select_resolver_for_registration;
use crate::util::output;

pub fn execute(domain: &str, port: u16, allow_custom_domain: bool) -> Result<()> {
    println!("{}", "ANTRA ALIAS".bold());
    println!();

    // DNS is case-insensitive — fold so MyApp and myapp share one route.
    let domain = domain.to_ascii_lowercase();
    let domain = domain.as_str();

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

    // Resolve the domain (add to hosts if needed).
    let resolver = select_resolver_for_registration(domain, allow_custom_domain)?;
    resolver.register(domain)?;
    println!(
        "  {} {}",
        "✓".green().bold(),
        format!("Domain resolved: {domain}").green()
    );

    // Warn when replacing an existing route instead of silently overwriting.
    if let Ok(resp) = send_command_sync(IpcPayload::ListRoutes) {
        if let IpcPayload::RoutesList(list) = resp.payload {
            if let Some(existing) = list.routes.iter().find(|r| r.domain == domain) {
                if existing.port != port {
                    println!(
                        "  {} {}",
                        "⚠".yellow().bold(),
                        format!(
                            "Domain {domain} is already routed to port {} — replacing with port {port}",
                            existing.port
                        )
                        .yellow()
                    );
                }
            }
        }
    }

    // Register route via IPC
    let resp = send_command_sync(IpcPayload::RegisterRoute(RegisterRouteRequest {
        domain: domain.to_string(),
        port,
        pid: None,
        managed: false,
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

    // Print the actual URL the user should visit (reflects fallback ports).
    output::print_route_url(domain);
    Ok(())
}
