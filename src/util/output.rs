use colored::Colorize;

pub fn print_success(msg: &str) {
    println!("  {} {}", "✓".green().bold(), msg);
}

pub fn print_error(msg: &str) {
    println!("  {} {}", "✗".red().bold(), msg);
}

pub fn print_warning(msg: &str) {
    println!("  {} {}", "⚠".yellow().bold(), msg);
}

pub fn print_header() {
    println!("{}", "ANTRA".bold().cyan());
    println!();
}

/// Print the URL the user should actually visit for a domain.
///
/// Reads the daemon's bound HTTPS port over IPC so the URL reflects
/// reality: `https://{domain}` on the default 443, or
/// `https://{domain}:{port}` when the daemon fell back (e.g. no sudo).
/// Falls back to the bare `https://` URL when the daemon can't be queried.
pub fn print_route_url(domain: &str) {
    println!();
    if let Ok(status) = crate::ipc::client::get_startup_status() {
        if status.https_port != 443 {
            println!(
                "  {} Note: HTTPS on port {} (port 443 unavailable — needs sudo or is in use)",
                "ℹ".cyan(),
                status.https_port
            );
            println!(
                "  {} To use port 443: {}",
                "ℹ".cyan(),
                "sudo antra proxy start".bold()
            );
            // Build clean URL — don't double-append .localhost
            let host = if domain.ends_with(".localhost") {
                domain.to_string()
            } else {
                format!("{domain}.localhost")
            };
            println!("  → https://{}:{}", host, status.https_port);
        } else {
            println!("  → https://{domain}");
        }
    } else {
        println!("  → https://{domain}");
    }
    println!();
}
