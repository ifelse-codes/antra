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

/// The URL the user should actually visit for a domain.
///
/// Reads the daemon's bound HTTPS port over IPC so the URL reflects
/// reality: `https://{domain}` on the default 443, or
/// `https://{domain}:{port}` when the daemon fell back (e.g. no sudo).
/// Falls back to the bare `https://` URL when the daemon can't be queried.
pub fn route_url(domain: &str) -> String {
    if let Ok(status) = crate::ipc::client::get_startup_status() {
        if status.https_port != 443 {
            return format!("https://{domain}:{}", status.https_port);
        }
    }
    format!("https://{domain}")
}

/// Print the URL the user should actually visit for a domain.
///
/// See [`route_url`]. Prints the fallback-port explainer alongside the URL
/// when the daemon isn't on 443.
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
            println!("  → {}", route_url(domain));
        } else {
            println!("  → https://{domain}");
        }
    } else {
        println!("  → https://{domain}");
    }
    println!();
}
