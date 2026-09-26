use anyhow::{Context, Result};
use colored::Colorize;

use crate::certs::store::CertStore;

/// Check if the Antra CA is trusted by the OS.
pub fn check_trust_status() -> Result<bool> {
    let Some(cert) = load_existing_ca()? else {
        return Ok(false);
    };

    #[cfg(target_os = "windows")]
    {
        match os_truststore::is_installed(&cert) {
            Ok(true) => Ok(true),
            Ok(false) => windows_current_user_contains(&cert),
            Err(system_error) => match windows_current_user_contains(&cert) {
                Ok(true) => Ok(true),
                Ok(false) => Err(anyhow::anyhow!(
                    "Failed to check the Windows system trust store: {system_error}"
                )),
                Err(user_error) => Err(anyhow::anyhow!(
                    "Failed to check Windows trust stores: system: {system_error}; CurrentUser Root: {user_error}"
                )),
            },
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        os_truststore::is_installed(&cert)
            .map_err(|e| anyhow::anyhow!("Failed to check trust store: {e}"))
    }
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
pub fn check_user_level_trust() -> Result<bool> {
    let cert = load_existing_ca()?;
    #[cfg(target_os = "macos")]
    {
        let Some(cert) = cert else {
            return Ok(false);
        };
        keychain_contains_cert(&cert)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cert;
        Ok(false)
    }
}

pub fn is_trusted_for_https() -> bool {
    check_trust_status().unwrap_or(false) || check_user_level_trust().unwrap_or(false)
}

fn load_existing_ca() -> Result<Option<os_truststore::Cert>> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("antra");
    let Some(ca_pem) = CertStore::read_existing_ca_pem(&config_dir)? else {
        return Ok(None);
    };
    validate_existing_ca_pem(&ca_pem)?;
    let cert = os_truststore::Cert::from_pem(&ca_pem)
        .context("Existing ca.pem is not a valid CA certificate")?;
    Ok(Some(cert))
}

fn validate_existing_ca_pem(pem: &str) -> Result<()> {
    let begins: Vec<_> = pem.match_indices("-----BEGIN CERTIFICATE-----").collect();
    let ends: Vec<_> = pem.match_indices("-----END CERTIFICATE-----").collect();
    if begins.len() != 1 || ends.len() != 1 {
        anyhow::bail!("Existing ca.pem must contain exactly one certificate");
    }
    let (begin, _) = begins[0];
    let (end, _) = ends[0];
    if begin >= end {
        anyhow::bail!("Existing ca.pem has reversed certificate markers");
    }
    let end_tag_end = end + "-----END CERTIFICATE-----".len();
    if !pem[..begin].trim().is_empty() || !pem[end_tag_end..].trim().is_empty() {
        anyhow::bail!("Existing ca.pem contains data outside its certificate");
    }
    Ok(())
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

#[cfg(target_os = "macos")]
fn keychain_contains_cert(cert: &os_truststore::Cert) -> Result<bool> {
    Ok(!matching_keychain_cert_hashes(cert)?.is_empty())
}

#[cfg(target_os = "macos")]
fn matching_keychain_cert_hashes(cert: &os_truststore::Cert) -> Result<Vec<String>> {
    let keychain = login_keychain_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine macOS login keychain path"))?;
    let common_name = cert
        .common_name()
        .ok_or_else(|| anyhow::anyhow!("Existing ca.pem has no Common Name"))?;
    let output = std::process::Command::new("security")
        .args(["find-certificate", "-c", common_name, "-a", "-Z", "-p"])
        .arg(&keychain)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to query the macOS login keychain")?;
    if output.status.code() == Some(44) {
        return Ok(Vec::new());
    }
    if !output.status.success() {
        anyhow::bail!(
            "Failed to query the macOS login keychain: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }

    let stdout = String::from_utf8(output.stdout)
        .context("macOS security returned a non-UTF-8 certificate listing")?;
    parse_matching_keychain_hashes(&stdout, &pem_payload(cert.pem()))
}

#[cfg(target_os = "macos")]
fn parse_matching_keychain_hashes(stdout: &str, want: &str) -> Result<Vec<String>> {
    let mut pairs: Vec<(String, String)> = Vec::new();
    let mut pending_hash = String::new();
    let mut block = String::new();
    let mut in_pem = false;
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(hash) = trimmed.strip_prefix("SHA-1 hash: ") {
            pending_hash = hash.trim().to_string();
        } else if trimmed == "-----BEGIN CERTIFICATE-----" {
            if in_pem {
                anyhow::bail!("macOS security returned a truncated certificate listing");
            }
            block.clear();
            block.push_str(line);
            block.push('\n');
            in_pem = true;
        } else if trimmed == "-----END CERTIFICATE-----" {
            if !in_pem {
                anyhow::bail!("macOS security returned an invalid certificate listing");
            }
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
    if in_pem {
        anyhow::bail!("macOS security returned a truncated certificate listing");
    }

    let mut matches = Vec::new();
    for (hash, pem_block) in pairs {
        if pem_payload(&pem_block) == want {
            if hash.is_empty() {
                anyhow::bail!("macOS security omitted the matching certificate hash");
            }
            matches.push(hash);
        }
    }
    Ok(matches)
}

#[cfg(target_os = "macos")]
fn delete_keychain_cert_by_hash(keychain_str: &str, hash: &str) -> Result<()> {
    let cmd = {
        let mut c = std::process::Command::new("security");
        c.args(["delete-certificate", "-Z", hash, keychain_str]);
        c
    };
    let status = run_security_mutation(cmd, std::time::Duration::from_secs(15))?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("security delete-certificate failed with exit code: {status}")
    }
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
            // Windows: fall back to CurrentUser scope (no elevation) instead
            // of hard-failing. Per-user trust is enough for the current
            // user's browsers.
            #[cfg(target_os = "windows")]
            {
                eprintln!("  Trying CurrentUser store instead (no elevation)...");
                match install_ca_windows_current_user(&ca.cert_pem) {
                    Ok(()) => {
                        println!(
                            "{}",
                            "  ✓ CA installed into CurrentUser Root store.".green()
                        );
                        println!(
                            "    {}",
                            "No elevation needed. HTTPS works for this Windows user.".dimmed()
                        );
                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("    CurrentUser install failed: {e}");
                    }
                }
            }
            eprintln!();
            eprintln!("    Try: {}", "sudo antra trust".bold());
            #[cfg(target_os = "macos")]
            {
                eprintln!("    Or: {}", "antra trust --user-level".bold());
            }
            #[cfg(target_os = "windows")]
            {
                eprintln!("    Or run in an Administrator terminal and retry.");
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
        if check_user_level_trust()? {
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

    // Windows: system scope needs elevation — fall back to CurrentUser
    // (per-user Root) so non-admin auto-trust still yields warning-free
    // HTTPS for the current user.
    #[cfg(target_os = "windows")]
    {
        if install_ca_windows_current_user(&ca.cert_pem).is_ok() {
            return Ok(());
        }
    }

    anyhow::bail!(
        "Could not install CA automatically. Try: {}",
        "sudo antra trust".bold()
    )
}

/// Install the Antra CA into the Windows CurrentUser Root store (no elevation).
///
/// Uses `certutil -user -addstore Root <ca.pem>`. The CA is local-only and
/// reversible (`certutil -user -delstore Root "Antra Local CA"`). Best-effort:
/// callers report the error with a manual retry hint.
#[cfg(target_os = "windows")]
fn install_ca_windows_current_user(ca_pem: &str) -> Result<()> {
    let dir = tempfile::tempdir()?;
    let cert_path = dir.path().join("antra-ca.pem");
    std::fs::write(&cert_path, ca_pem)?;
    let output = std::process::Command::new("certutil")
        .args(["-user", "-addstore", "Root"])
        .arg(&cert_path)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    let detail = format!(
        "{} {}",
        String::from_utf8_lossy(&output.stdout).trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    anyhow::bail!("certutil CurrentUser install failed: {}", detail.trim())
}

#[cfg(target_os = "windows")]
fn windows_current_user_contains(cert: &os_truststore::Cert) -> Result<bool> {
    let store = schannel::cert_store::CertStore::open_current_user("Root")
        .context("Failed to open the Windows CurrentUser Root store")?;
    Ok(store.certs().any(|entry| entry.to_der() == cert.der()))
}

#[cfg(target_os = "windows")]
fn remove_windows_current_user_ca(cert: &os_truststore::Cert) -> Result<()> {
    let store = schannel::cert_store::CertStore::open_current_user("Root")
        .context("Failed to open the Windows CurrentUser Root store")?;
    let matching: Vec<_> = store
        .certs()
        .filter(|entry| entry.to_der() == cert.der())
        .collect();
    for entry in matching {
        entry
            .delete()
            .context("Failed to remove the CA from the Windows CurrentUser Root store")?;
    }
    drop(store);
    if windows_current_user_contains(cert)? {
        anyhow::bail!("CA remains in the Windows CurrentUser Root store");
    }
    Ok(())
}

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
        if keychain_contains_cert(&os_cert)? {
            println!(
                "{}",
                "  Antra CA is already trusted via your login keychain (user-level, no sudo)."
                    .green()
            );
            return Ok(());
        }

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
    let os_cert =
        os_truststore::Cert::from_pem(&ca.cert_pem).context("Failed to parse CA certificate")?;
    if keychain_contains_cert(&os_cert)? {
        return Ok(());
    }

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

/// Remove the Antra CA from the OS trust store (and macOS login keychain).
/// Prompts the user before making system changes.
pub fn remove_ca() -> Result<()> {
    let Some(cert) = load_existing_ca()? else {
        println!(
            "{}",
            "  ! ca.pem not found; trust cleanup skipped.".yellow()
        );
        return Ok(());
    };
    if !ca_is_installed(&cert)? {
        println!(
            "{}",
            "  Antra CA is not currently trusted by the system.".yellow()
        );
        return Ok(());
    }

    println!("  Antra will remove its local CA certificate from your system trust store.");
    println!("  Applicable system and user-level trust entries will be removed.");
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

    remove_ca_exact(&cert)
}

pub(crate) fn remove_ca_noninteractive() -> Result<()> {
    let Some(cert) = load_existing_ca()? else {
        println!(
            "{}",
            "  ! ca.pem not found; trust cleanup skipped.".yellow()
        );
        return Ok(());
    };
    remove_ca_exact(&cert)
}

fn ca_is_installed(cert: &os_truststore::Cert) -> Result<bool> {
    let system_installed = os_truststore::is_installed(cert)
        .map_err(|e| anyhow::anyhow!("Failed to check trust store: {e}"))?;
    #[cfg(target_os = "macos")]
    let user_installed = keychain_contains_cert(cert)?;
    #[cfg(target_os = "windows")]
    let user_installed = windows_current_user_contains(cert)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let user_installed = false;
    Ok(system_installed || user_installed)
}

fn remove_ca_exact(cert: &os_truststore::Cert) -> Result<()> {
    let mut failures = Vec::new();
    if let Err(e) = remove_system_ca_exact(cert) {
        failures.push(format!("system store: {e:#}"));
    }
    #[cfg(target_os = "macos")]
    if let Err(e) = remove_macos_login_keychain_ca_exact(cert) {
        failures.push(format!("login keychain: {e:#}"));
    }
    #[cfg(target_os = "windows")]
    if let Err(e) = remove_windows_current_user_ca(cert) {
        failures.push(format!("CurrentUser Root: {e:#}"));
    }

    if failures.is_empty() {
        println!(
            "{}",
            "  ✓ Current Antra CA is absent from all applicable trust stores.".green()
        );
        Ok(())
    } else {
        anyhow::bail!("CA trust removal incomplete: {}", failures.join("; "))
    }
}

fn remove_system_ca_exact(cert: &os_truststore::Cert) -> Result<()> {
    let installed = os_truststore::is_installed(cert)
        .map_err(|e| anyhow::anyhow!("Failed to verify the system trust store: {e}"))?;
    if !installed {
        return Ok(());
    }
    match os_truststore::uninstall(cert) {
        Ok(()) => {}
        Err(os_truststore::TrustError::NeedsElevation { detail }) => {
            anyhow::bail!("elevated privileges required: {detail}")
        }
        Err(os_truststore::TrustError::InteractiveAuthRequired) => {
            anyhow::bail!("interactive keychain authorization required")
        }
        Err(e) => return Err(anyhow::anyhow!("system trust removal failed: {e}")),
    }
    let still_installed = os_truststore::is_installed(cert)
        .map_err(|e| anyhow::anyhow!("Failed to verify system trust removal: {e}"))?;
    if still_installed {
        anyhow::bail!("CA remains in the system trust store");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn remove_macos_login_keychain_ca_exact(cert: &os_truststore::Cert) -> Result<()> {
    let hashes = matching_keychain_cert_hashes(cert)?;
    if hashes.is_empty() {
        return Ok(());
    }
    let keychain = login_keychain_path()
        .ok_or_else(|| anyhow::anyhow!("Could not determine macOS login keychain path"))?;
    let keychain_str = keychain
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("macOS login keychain path is not valid UTF-8"))?;
    for hash in hashes {
        delete_keychain_cert_by_hash(keychain_str, &hash)?;
    }
    let remaining = matching_keychain_cert_hashes(cert)?;
    if !remaining.is_empty() {
        anyhow::bail!("CA remains in the macOS login keychain");
    }
    Ok(())
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
mod ca_pem_tests {
    use super::*;

    #[test]
    fn existing_ca_pem_requires_one_certificate() {
        let ca = crate::certs::ca::generate_ca().unwrap();
        validate_existing_ca_pem(&ca.cert_pem).unwrap();
        assert!(validate_existing_ca_pem(&format!("{}{}", ca.cert_pem, ca.cert_pem)).is_err());
        assert!(validate_existing_ca_pem(&format!("junk\n{}", ca.cert_pem)).is_err());
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn keychain_match_ignores_same_name_certificate() {
        let current = crate::certs::ca::generate_ca().unwrap();
        let other = crate::certs::ca::generate_ca().unwrap();
        let listing = format!(
            "SHA-1 hash: current\n{}\nSHA-1 hash: other\n{}",
            current.cert_pem, other.cert_pem
        );

        assert_eq!(
            parse_matching_keychain_hashes(&listing, &pem_payload(&current.cert_pem)).unwrap(),
            vec!["current".to_string()]
        );
    }

    #[test]
    fn keychain_match_rejects_truncated_listing() {
        let current = crate::certs::ca::generate_ca().unwrap();
        let listing = format!(
            "SHA-1 hash: current\n{}",
            current.cert_pem.replace("-----END CERTIFICATE-----\n", "")
        );

        assert!(parse_matching_keychain_hashes(&listing, &pem_payload(&current.cert_pem)).is_err());
    }
}
