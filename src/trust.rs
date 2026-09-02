use anyhow::{Context, Result};
use colored::Colorize;

use crate::certs::store::CertStore;

/// Check if the Antra CA is trusted by the OS.
pub fn check_trust_status() -> Result<bool> {
    let store = CertStore::new()?;
    let ca = store.get_or_create_ca()?;
    let os_cert = os_truststore::Cert::from_pem(&ca.cert_pem)
        .context("Failed to parse CA certificate for trust check")?;
    let installed = os_truststore::is_installed(&os_cert)
        .map_err(|e| anyhow::anyhow!("Failed to check trust store: {e}"))?;
    Ok(installed)
}

/// Install the Antra CA into the OS trust store.
/// Prompts the user before making system changes.
pub fn install_ca() -> Result<()> {
    let store = CertStore::new()?;
    let ca = store.get_or_create_ca()?;
    let os_cert =
        os_truststore::Cert::from_pem(&ca.cert_pem).context("Failed to parse CA certificate")?;

    // Check if already installed
    let already_installed = os_truststore::is_installed(&os_cert)
        .map_err(|e| anyhow::anyhow!("Failed to check trust store: {e}"))?;

    if already_installed {
        println!("{}", "  Antra CA is already trusted by the system.".green());
        return Ok(());
    }

    // Prompt user before modifying trust store
    println!("  Antra needs to install a local CA certificate into your system trust store.");
    println!(
        "  This allows HTTPS for custom domains like {}.",
        "myapp.test".cyan()
    );
    println!();
    print!(
        "  {} ",
        "Install CA into system trust store? [y/N]".yellow()
    );
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input != "y" && input != "yes" {
        println!(
            "  {}",
            "Skipped. HTTPS for custom domains will show cert warnings.".dimmed()
        );
        return Ok(());
    }

    // Attempt install
    match os_truststore::install(&os_cert) {
        Ok(report) => {
            println!(
                "{}",
                "  ✓ CA certificate installed into system trust store.".green()
            );
            if let Some(detail) = report_detail(&report) {
                println!("    {detail}");
            }
            Ok(())
        }
        Err(os_truststore::TrustError::NeedsElevation { detail }) => {
            eprintln!("{}", "  ✗ Elevated privileges required.".red());
            eprintln!("    {detail}");
            eprintln!();
            eprintln!("    Run with sudo, or follow the manual steps above.");
            anyhow::bail!("Elevation required to install CA")
        }
        Err(os_truststore::TrustError::InteractiveAuthRequired) => {
            eprintln!(
                "{}",
                "  ✗ Interactive authentication required (macOS GUI prompt).".red()
            );
            eprintln!("    This command needs a terminal with GUI access.");
            anyhow::bail!("Interactive auth required")
        }
        Err(os_truststore::TrustError::StoreToolMissing { hint }) => {
            eprintln!("{}", "  ✗ Trust store tool not installed.".red());
            eprintln!("    {hint}");
            anyhow::bail!("Store tool missing")
        }
        Err(os_truststore::TrustError::Unsupported) => {
            eprintln!(
                "{}",
                "  ✗ Unsupported platform for trust store modification.".red()
            );
            anyhow::bail!("Unsupported platform")
        }
        Err(e) => {
            eprintln!("{}", format!("  ✗ Failed to install CA: {e}").red());
            anyhow::bail!("Trust install failed: {e}")
        }
    }
}

/// Remove the Antra CA from the OS trust store.
/// Prompts the user before making system changes.
pub fn remove_ca() -> Result<()> {
    let store = CertStore::new()?;
    let ca = store.get_or_create_ca()?;
    let os_cert =
        os_truststore::Cert::from_pem(&ca.cert_pem).context("Failed to parse CA certificate")?;

    // Check if installed
    let installed = os_truststore::is_installed(&os_cert)
        .map_err(|e| anyhow::anyhow!("Failed to check trust store: {e}"))?;

    if !installed {
        println!(
            "{}",
            "  Antra CA is not currently trusted by the system.".yellow()
        );
        return Ok(());
    }

    // Prompt user before modifying trust store
    println!("  Antra will remove its local CA certificate from your system trust store.");
    println!("  HTTPS for custom domains will show cert warnings after removal.");
    println!();
    print!("  {} ", "Remove CA from system trust store? [y/N]".yellow());
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input != "y" && input != "yes" {
        println!("  {}", "Skipped. CA remains trusted.".dimmed());
        return Ok(());
    }

    // Attempt removal
    match os_truststore::uninstall(&os_cert) {
        Ok(()) => {
            println!(
                "{}",
                "  ✓ CA certificate removed from system trust store.".green()
            );
            Ok(())
        }
        Err(os_truststore::TrustError::NeedsElevation { detail }) => {
            eprintln!("{}", "  ✗ Elevated privileges required.".red());
            eprintln!("    {detail}");
            eprintln!();
            eprintln!("    Run with sudo, or follow the manual steps above.");
            anyhow::bail!("Elevation required to remove CA")
        }
        Err(os_truststore::TrustError::InteractiveAuthRequired) => {
            eprintln!(
                "{}",
                "  ✗ Interactive authentication required (macOS GUI prompt).".red()
            );
            eprintln!("    This command needs a terminal with GUI access.");
            anyhow::bail!("Interactive auth required")
        }
        Err(e) => {
            eprintln!("{}", format!("  ✗ Failed to remove CA: {e}").red());
            anyhow::bail!("Trust removal failed: {e}")
        }
    }
}

/// Format an install report into a human-readable detail string.
fn report_detail(report: &os_truststore::Report) -> Option<String> {
    match report {
        os_truststore::Report::AlreadyInstalled => {
            Some("Certificate was already in the trust store.".to_string())
        }
        os_truststore::Report::Installed => None,
        os_truststore::Report::InstalledNotTrusted { reason } => {
            Some(format!("Installed but trust not confirmed: {reason}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_trust_status_runs() {
        // This just verifies the function doesn't panic.
        // Actual trust status depends on the environment.
        let _ = check_trust_status();
    }
}
