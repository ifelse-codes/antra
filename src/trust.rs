use anyhow::{Context, Result};
use colored::Colorize;

use crate::certs::store::CertStore;

/// Common Name of the Antra local root CA. Must match `certs::ca`.
/// macOS-only: used for the login-keychain trust lookup.
#[cfg(target_os = "macos")]
pub const CA_COMMON_NAME: &str = "Antra Local CA";

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

/// Check if the Antra CA is trusted at user level (no sudo).
///
/// On macOS, `antra trust --user-level` installs into the login keychain,
/// which the system-store check above does not see. Returns false on
/// other platforms (user-level install is macOS-only).
///
/// Compares certificate bytes, not just the Common Name: if the CA was
/// regenerated since it was trusted, the keychain holds a *stale* cert and
/// this correctly reports untrusted (re-run `trust --user-level` to fix).
pub fn check_user_level_trust() -> bool {
    #[cfg(target_os = "macos")]
    {
        let ca_pem = CertStore::new()
            .ok()
            .and_then(|s| std::fs::read_to_string(s.config_dir.join("ca.pem")).ok());
        let Some(ca_pem) = ca_pem else {
            return false;
        };
        keychain_contains_cert(&ca_pem)
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// True when HTTPS will work with no warnings: system trust store OR
/// macOS user-level login-keychain trust. Prefer this over
/// `check_trust_status()` for "do we need to do anything?" decisions —
/// otherwise macOS users get re-prompted despite already-trusted HTTPS.
pub fn is_trusted_for_https() -> bool {
    check_trust_status().unwrap_or(false) || check_user_level_trust()
}

/// Path to the macOS login keychain.
#[cfg(target_os = "macos")]
fn login_keychain_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/Keychains/login.keychain-db"))
}

/// Normalize a PEM document to its base64 payload (no headers/whitespace),
/// so certificates can be compared by bytes regardless of line wrapping.
#[cfg(target_os = "macos")]
fn pem_payload(pem: &str) -> String {
    pem.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("-----"))
        .collect()
}

/// True when the login keychain contains a certificate byte-identical to
/// `ca_pem`. Same Common Name is not enough — a regenerated CA shares the
/// name but fails TLS verification against the old keychain entry.
///
/// Uses `-a` to compare EVERY same-name entry: `find-certificate` without
/// it returns only the first match, which may be a stale duplicate while
/// the current cert sits further down the list.
#[cfg(target_os = "macos")]
fn keychain_contains_cert(ca_pem: &str) -> bool {
    let Some(keychain) = login_keychain_path() else {
        return false;
    };
    let output = std::process::Command::new("security")
        .args([
            "find-certificate",
            "-c",
            CA_COMMON_NAME,
            "-a",
            "-p",
            keychain.to_str().unwrap_or_default(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let want = pem_payload(ca_pem);
    // `-p` concatenates PEM blocks; split on the END marker and compare each.
    stdout
        .split("-----END CERTIFICATE-----")
        .any(|block| !pem_payload(block).is_empty() && pem_payload(block) == want)
}

/// SHA-1 hashes of every login-keychain certificate carrying our CA's
/// Common Name.
#[cfg(target_os = "macos")]
fn keychain_ca_hashes() -> Vec<String> {
    let Some(keychain) = login_keychain_path() else {
        return Vec::new();
    };
    let output = std::process::Command::new("security")
        .args([
            "find-certificate",
            "-c",
            CA_COMMON_NAME,
            "-a",
            "-Z",
            keychain.to_str().unwrap_or_default(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("SHA-1 hash: ")
                .map(|h| h.trim().to_string())
        })
        .collect()
}

/// Delete one login-keychain certificate by SHA-1 hash.
///
/// Best-effort with a short timeout: `security` can pop a GUI auth dialog
/// (locked keychain) that never resolves headless — never let pruning hang
/// a trust command. Failures are ignored; the subsequent `add-trusted-cert`
/// succeeding is what matters.
#[cfg(target_os = "macos")]
fn delete_keychain_cert_by_hash(keychain_str: &str, hash: &str) {
    let cmd = {
        let mut c = std::process::Command::new("security");
        c.args(["delete-certificate", "-Z", hash, keychain_str]);
        c
    };
    let _ = run_security_mutation(cmd, std::time::Duration::from_secs(15));
}

/// Run a mutating `security` command with a bounded wait.
///
/// Reads (`find-certificate`) return fast; MUTATIONS (`add-trusted-cert`,
/// `delete-certificate`) may pop a GUI approval dialog when the keychain
/// wants auth. With no GUI session that dialog never resolves, so waiting
/// forever would hang the CLI with zero output. Instead, kill the child
/// after `timeout` and report that GUI approval may be needed.
#[cfg(target_os = "macos")]
fn run_security_mutation(
    mut cmd: std::process::Command,
    timeout: std::time::Duration,
) -> Result<std::process::ExitStatus> {
    use anyhow::Context;
    let mut child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("Failed to run `security`")?;
    let start = std::time::Instant::now();
    loop {
        match child.try_wait().context("Failed to poll `security`")? {
            Some(status) => return Ok(status),
            None if start.elapsed() > timeout => {
                let _ = child.kill();
                anyhow::bail!(
                    "`security` timed out after {}s — macOS may be waiting for \
                     Keychain approval in a GUI dialog. Unlock your login \
                     keychain (Keychain Access) and retry.",
                    timeout.as_secs()
                );
            }
            None => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}

/// Remove keychain certificates carrying our CA's Common Name.
///
/// Used before (re-)installing so `trust` stays idempotent: without this,
/// every re-run appends a duplicate entry, and a regenerated CA leaves a
/// stale entry that shadows nothing but confuses.
///
/// Deletes by SHA-1 hash, one entry at a time: `delete-certificate -c`
/// refuses with "ambiguous, matches more than one certificate" as soon as
/// duplicates exist — exactly when cleanup is needed most. Failures are
/// ignored — the subsequent `add-trusted-cert` succeeding is what matters.
#[cfg(target_os = "macos")]
fn remove_stale_keychain_certs() {
    let Some(keychain) = login_keychain_path() else {
        return;
    };
    let keychain_str = keychain.to_str().unwrap_or_default();
    for hash in keychain_ca_hashes() {
        delete_keychain_cert_by_hash(keychain_str, &hash);
    }
}

/// Remove same-name keychain entries that do NOT match the current CA,
/// keeping the current one. Returns the number removed.
///
/// Called on the already-trusted path so re-running `trust` converges the
/// keychain to exactly one entry instead of letting stale duplicates from
/// past CA regenerations accumulate.
#[cfg(target_os = "macos")]
fn remove_other_keychain_certs(keep_pem: &str) -> usize {
    let Some(keychain) = login_keychain_path() else {
        return 0;
    };
    let keychain_str = keychain.to_str().unwrap_or_default();
    let output = std::process::Command::new("security")
        .args([
            "find-certificate",
            "-c",
            CA_COMMON_NAME,
            "-a",
            "-Z",
            "-p",
            keychain_str,
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();
    let Ok(output) = output else {
        return 0;
    };
    // Pair each PEM block with the SHA-1 hash printed just above it.
    // Only lines between the BEGIN/END markers belong to the block —
    // the SHA-256/SHA-1 header lines must not pollute the payload.
    let keep = pem_payload(keep_pem);
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut pending_hash = String::new();
    let mut block = String::new();
    let mut in_pem = false;
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if let Some(hash) = trimmed.strip_prefix("SHA-1 hash: ") {
            pending_hash = hash.trim().to_string();
        } else if trimmed == "-----BEGIN CERTIFICATE-----" {
            block.clear();
            block.push_str(line);
            block.push('\n');
            in_pem = true;
        } else if trimmed == "-----END CERTIFICATE-----" {
            block.push_str(line);
            pairs.push((
                std::mem::take(&mut pending_hash),
                std::mem::take(&mut block),
            ));
            in_pem = false;
        } else if in_pem {
            block.push_str(line);
            block.push('\n');
        }
    }
    let mut removed = 0;
    for (hash, pem_block) in &pairs {
        if hash.is_empty() || pem_payload(pem_block) == keep {
            continue;
        }
        delete_keychain_cert_by_hash(keychain_str, hash);
        removed += 1;
    }
    removed
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

    // macOS non-root: the system store would demand sudo — offer the login
    // keychain directly (no sudo, same warning-free HTTPS) instead of
    // failing through an elevation error mid-flow.
    #[cfg(target_os = "macos")]
    {
        let non_root = unsafe { libc::geteuid() != 0 };
        if non_root {
            return install_ca_user_level_prompted();
        }
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
            eprintln!(
                "{}",
                "  ✗ Elevated privileges required for system trust store.".red()
            );
            eprintln!("    {detail}");
            eprintln!();
            // Try user-level keychain on macOS
            #[cfg(target_os = "macos")]
            {
                eprintln!("  Trying user login keychain instead (no sudo)...");
                match install_ca_user_level_silent(&ca) {
                    Ok(()) => {
                        println!("{}", "  ✓ CA installed into user login keychain.".green());
                        println!(
                            "    {}",
                            "No sudo required. HTTPS for custom domains is ready.".dimmed()
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("    User keychain install failed: {e}");
                    }
                }
            }
            eprintln!();
            eprintln!("    Try: {}", "sudo antra trust".bold());
            #[cfg(target_os = "macos")]
            {
                eprintln!("    Or: {}", "antra trust --user-level".bold());
            }
            anyhow::bail!("Elevation required to install CA")
        }
        Err(os_truststore::TrustError::InteractiveAuthRequired) => {
            eprintln!(
                "{}",
                "  ✗ Interactive authentication required (macOS GUI prompt).".red()
            );
            eprintln!("    This command needs a terminal with GUI access.");
            eprintln!("    Try: {}", "sudo antra trust".bold());
            #[cfg(target_os = "macos")]
            {
                eprintln!("    Or: {}", "antra trust --user-level".bold());
            }
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

/// Install the Antra CA into the OS trust store without prompting.
/// Used by `antra run` for first-time auto-trust and by the installer.
///
/// macOS: user-level login keychain FIRST — silent, no sudo, no GUI auth
/// prompt, and sufficient for warning-free HTTPS. Automatic paths must
/// never pop a system auth dialog, so the system store is not attempted
/// here (use `sudo antra trust` for system-wide trust explicitly).
pub fn install_ca_noninteractive() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let store = CertStore::new()?;
        let ca = store.get_or_create_ca()?;
        if check_user_level_trust() {
            return Ok(());
        }
        if install_ca_user_level_silent(&ca).is_ok() {
            return Ok(());
        }
        anyhow::bail!(
            "Could not install CA automatically. Try: {}",
            "antra trust --user-level".bold(),
        )
    }

    #[cfg(not(target_os = "macos"))]
    {
        install_ca_noninteractive_system()
    }
}

/// System-store auto-install for non-macOS platforms.
/// Tries a silent system install; bails with a manual hint otherwise.
#[cfg(not(target_os = "macos"))]
fn install_ca_noninteractive_system() -> Result<()> {
    let store = CertStore::new()?;
    let ca = store.get_or_create_ca()?;
    let os_cert =
        os_truststore::Cert::from_pem(&ca.cert_pem).context("Failed to parse CA certificate")?;

    // Check if already installed
    let already_installed = os_truststore::is_installed(&os_cert)
        .map_err(|e| anyhow::anyhow!("Failed to check trust store: {e}"))?;

    if already_installed {
        return Ok(());
    }

    // Try default install (may work without elevation on some systems)
    if os_truststore::install(&os_cert).is_ok() {
        return Ok(());
    }

    anyhow::bail!(
        "Could not install CA automatically. Try: {}",
        "sudo antra trust".bold()
    )
}

/// Prompt, then install the Antra CA into the user's login keychain.
/// macOS non-root entry point for interactive `antra trust`: no sudo, no
/// elevation errors — just one question defaulting to yes. The CA is
/// local-only and reversible (`antra trust --remove` does not yet cover
/// the keychain; re-running is idempotent).
#[cfg(target_os = "macos")]
fn install_ca_user_level_prompted() -> Result<()> {
    println!("  Antra needs a local CA certificate so HTTPS works with no warnings.");
    println!(
        "  This installs into your {} (no sudo, local-only).",
        "login keychain".cyan()
    );
    println!();
    print!(
        "  {} ",
        "Install CA into your login keychain? [Y/n]".yellow()
    );
    use std::io::Write;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let input = input.trim().to_lowercase();

    if input == "n" || input == "no" {
        println!(
            "  {}",
            "Skipped. HTTPS will show cert warnings until you run `antra trust`.".dimmed()
        );
        return Ok(());
    }

    install_ca_user_level()
}

/// Install the Antra CA into the user's login keychain (no sudo needed).
/// macOS only: installs to ~/Library/Keychains/login.keychain-db
pub fn install_ca_user_level() -> Result<()> {
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

    // macOS: install to user login keychain
    #[cfg(target_os = "macos")]
    {
        // Idempotent: the exact cert is already there — don't append a duplicate.
        // Prune stale same-name entries so the keychain converges to one.
        if keychain_contains_cert(&ca.cert_pem) {
            let pruned = remove_other_keychain_certs(&ca.cert_pem);
            println!(
                "{}",
                "  Antra CA is already trusted via your login keychain (user-level, no sudo)."
                    .green()
            );
            if pruned > 0 {
                println!(
                    "    {}",
                    format!("Removed {pruned} stale duplicate(s) from the login keychain.")
                        .dimmed()
                );
            }
            return Ok(());
        }
        // Otherwise drop same-name entries first: a regenerated CA shares the
        // Common Name, and leaving the stale cert behind both duplicates the
        // entry and masks the trust state.
        remove_stale_keychain_certs();

        let temp_cert = tempfile::NamedTempFile::new()?;
        std::fs::write(temp_cert.path(), &ca.cert_pem)?;

        let keychain_path = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
            .join("Library/Keychains/login.keychain-db");

        let mut cmd = std::process::Command::new("security");
        cmd.args([
            "add-trusted-cert",
            "-r",
            "trustRoot",
            "-k",
            keychain_path.to_str().unwrap(),
            temp_cert.path().to_str().unwrap(),
        ]);
        // Bounded wait: a locked keychain pops a GUI approval dialog that
        // never resolves headless — fail with a hint instead of hanging.
        // Generous timeout: an interactive user may be approving at the GUI.
        let status = run_security_mutation(cmd, std::time::Duration::from_secs(120));

        match status {
            Ok(s) if s.success() => {
                println!(
                    "{}",
                    "  ✓ CA certificate installed into user login keychain.".green()
                );
                println!(
                    "    {}",
                    "No sudo required. HTTPS for custom domains is ready.".dimmed()
                );
                Ok(())
            }
            Ok(s) => {
                anyhow::bail!(
                    "Failed to install to user keychain (exit code: {}). \
                     Try: sudo antra trust",
                    s
                );
            }
            Err(e) => {
                anyhow::bail!("Failed to install to user keychain: {e}");
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On Linux/Windows, user-level trust store is not straightforward
        // Fall back to suggesting sudo
        println!(
            "{}",
            "  User-level trust install is only supported on macOS.".yellow()
        );
        println!("  On this platform, try: {}", "sudo antra trust".bold());
        anyhow::bail!("User-level trust install not supported on this platform")
    }
}

/// Install CA into user login keychain without output (for noninteractive fallback).
#[cfg(target_os = "macos")]
fn install_ca_user_level_silent(ca: &crate::certs::ca::CaCert) -> Result<()> {
    // Idempotent: skip when the exact cert is already trusted (pruning
    // stale same-name entries); replace stale entries (e.g. after CA
    // regeneration) otherwise.
    if keychain_contains_cert(&ca.cert_pem) {
        remove_other_keychain_certs(&ca.cert_pem);
        return Ok(());
    }
    remove_stale_keychain_certs();

    let temp_cert = tempfile::NamedTempFile::new()?;
    std::fs::write(temp_cert.path(), &ca.cert_pem)?;

    let keychain_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?
        .join("Library/Keychains/login.keychain-db");

    let mut cmd = std::process::Command::new("security");
    cmd.args([
        "add-trusted-cert",
        "-r",
        "trustRoot",
        "-k",
        keychain_path.to_str().unwrap(),
        temp_cert.path().to_str().unwrap(),
    ]);

    // Short timeout: automatic paths must stay bounded — on failure the
    // caller prints a manual `antra trust --user-level` hint.
    match run_security_mutation(cmd, std::time::Duration::from_secs(30)) {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => anyhow::bail!("security command failed with exit code: {s}"),
        Err(e) => anyhow::bail!("Automatic keychain install failed: {e}"),
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
            eprintln!("    Try: {}", "sudo antra trust --remove".bold());
            anyhow::bail!("Elevation required to remove CA")
        }
        Err(os_truststore::TrustError::InteractiveAuthRequired) => {
            eprintln!(
                "{}",
                "  ✗ Interactive authentication required (macOS GUI prompt).".red()
            );
            eprintln!("    This command needs a terminal with GUI access.");
            eprintln!("    Try: {}", "sudo antra trust --remove".bold());
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
