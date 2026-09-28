use anyhow::Result;
use colored::Colorize;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ServiceCommands {
    /// Install Antra as a system service
    Install,
    /// Show service status
    Status,
    /// Uninstall Antra service
    Uninstall,
}

pub fn execute(command: ServiceCommands) -> Result<()> {
    match command {
        ServiceCommands::Install => install_service(),
        ServiceCommands::Status => service_status(),
        ServiceCommands::Uninstall => uninstall_service(),
    }
}

fn install_service() -> Result<()> {
    println!("{}", "ANTRA SERVICE INSTALL".bold());
    println!();

    #[cfg(target_os = "macos")]
    {
        install_launchd()
    }

    #[cfg(target_os = "linux")]
    {
        install_systemd()
    }

    #[cfg(target_os = "windows")]
    {
        install_windows_service()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Service install is only supported on macOS, Linux, and Windows".red()
        );
        Ok(())
    }
}

/// Windows service install via `sc.exe` (manual start, `AntraDaemon`).
///
/// Manual start (not auto) so a logout/boot never surprises with bound :443.
/// Requires elevation; without it `sc.exe` fails and we print the
/// elevated retry instead of a stack trace.
#[cfg(target_os = "windows")]
fn install_windows_service() -> Result<()> {
    let antra_path = std::env::current_exe()?;
    let bin_path = format!("\"{}\" proxy start", antra_path.display());

    let output = std::process::Command::new("sc.exe")
        .args([
            "create",
            "AntraDaemon",
            &format!("binPath= {bin_path}"),
            "start=",
            "demand",
        ])
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        if stdout.contains("Access is denied") || stderr.contains("Access is denied") {
            println!(
                "  {} {}",
                "✗".red().bold(),
                "Elevation required: run in an Administrator terminal,".red()
            );
            println!("    then retry: antra service install");
            return Ok(());
        }
        println!(
            "  {} Failed to create service: {} {}",
            "✗".red().bold(),
            stdout.trim(),
            stderr.trim()
        );
        return Ok(());
    }
    let _ = std::process::Command::new("sc.exe")
        .args([
            "description",
            "AntraDaemon",
            "Antra local development proxy (manual start)",
        ])
        .output();
    println!(
        "  {} Service 'AntraDaemon' installed (manual start)",
        "✓".green().bold()
    );
    println!("  {}", "Start it with: sc.exe start AntraDaemon".dimmed());
    println!("  {}", "Or keep using: antra proxy start".dimmed());
    println!();
    // The service runs as SYSTEM: its CA and aliases.json live under the
    // SYSTEM profile, not the installing user's. Without this note the mode
    // looks broken end-to-end (different CA than `antra trust` installed,
    // none of the user's aliases). Say it up front instead.
    println!(
        "  {} {}",
        "⚠".yellow().bold(),
        "Limitation: the service runs as SYSTEM, so it uses the SYSTEM".yellow()
    );
    println!(
        "  {}",
        "  profile's CA and aliases — not yours. Trust/aliases you created".dimmed()
    );
    println!(
        "  {}",
        "  as yourself won't apply to it (expect TLS warnings). For".dimmed()
    );
    println!(
        "  {}",
        "  single-user dev, prefer `antra proxy start`.".dimmed()
    );
    println!();
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_launchd() -> Result<()> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let launch_agents_dir = home_dir.join("Library/LaunchAgents");
    let plist_path = launch_agents_dir.join("com.antra.proxy.plist");
    let antra_path = std::env::current_exe()?;

    // Create LaunchAgents directory if it doesn't exist
    std::fs::create_dir_all(&launch_agents_dir)?;

    // Same file `antra proxy start` and `antra logs` use. It used to point at
    // `~/.config/antra/daemon.log`, which on macOS is a *different* path from
    // the one the CLI writes (data_local_dir → ~/Library/Application
    // Support), so a service-managed daemon's output was unreachable.
    let log_path = crate::util::logs::daemon_log_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let plist_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.antra.proxy</string>

    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>proxy</string>
        <string>start</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>{}</string>

    <key>StandardErrorPath</key>
    <string>{}</string>
</dict>
</plist>"#,
        antra_path.display(),
        log_path.display(),
        log_path.display()
    );

    std::fs::write(&plist_path, &plist_content)?;

    println!(
        "  {} Created launchd plist: {}",
        "✓".green().bold(),
        plist_path.display()
    );

    // Load the service
    let output = std::process::Command::new("launchctl")
        .args(["load", "-w", &plist_path.to_string_lossy()])
        .output()?;

    if output.status.success() {
        println!("  {} Service loaded and enabled", "✓".green().bold());
        println!();
        println!(
            "  {}",
            "Antra proxy will start automatically on login".dimmed()
        );
        println!("  {}", "URLs will survive reboots".dimmed());
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "  {} Failed to load service: {}",
            "✗".red().bold(),
            stderr.trim()
        );
    }

    println!();
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_systemd() -> Result<()> {
    let service_path = unit_path()?;
    let service_dir = service_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine the service directory"))?
        .to_path_buf();
    let antra_path = std::env::current_exe()?;

    // Create systemd directory if it doesn't exist
    std::fs::create_dir_all(&service_dir)?;

    let service_content = format!(
        r#"[Unit]
Description=Antra Local Development Proxy
After=network.target

[Service]
Type=simple
ExecStart={} proxy start
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
"#,
        antra_path.display()
    );

    std::fs::write(&service_path, &service_content)?;

    println!(
        "  {} Created systemd service: {}",
        "✓".green().bold(),
        service_path.display()
    );

    // Enable and start the service
    let output = std::process::Command::new("systemctl")
        .args(["--user", "enable", "antra-proxy"])
        .output()?;

    if output.status.success() {
        println!("  {} Service enabled", "✓".green().bold());

        // Start the service
        let start_output = std::process::Command::new("systemctl")
            .args(["--user", "start", "antra-proxy"])
            .output()?;

        if start_output.status.success() {
            println!("  {} Service started", "✓".green().bold());
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        println!(
            "  {} Failed to enable service: {}",
            "✗".red().bold(),
            stderr.trim()
        );
    }

    println!();
    println!(
        "  {}",
        "Antra proxy will start automatically on login".dimmed()
    );
    println!("  {}", "URLs will survive reboots".dimmed());
    println!();
    Ok(())
}

/// Where `antra service install` puts the systemd user unit, and where
/// `antra service status` looks for it.
///
/// Shared so the two cannot drift. It is deliberately *not* systemd's default
/// search path — see the note on `LinuxServiceState` — so the file's presence
/// is the only trustworthy statement about whether install ever happened.
#[cfg(target_os = "linux")]
fn unit_path() -> Result<std::path::PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    Ok(home_dir
        .join(".config/antra")
        .join("systemd/user")
        .join("antra-proxy.service"))
}

/// What `antra service status` should report on Linux.
///
/// Split out from the subprocess calls so the mapping is testable — the file
/// had no tests at all — and so the reasoning is stated once.
///
/// Deliberately *not* `#[cfg(target_os = "linux")]`. The mapping is pure
/// string logic with no platform dependency, and gating it would mean the most
/// subtle code in this file was exercised on exactly one platform and never
/// on a developer's Mac.
#[derive(Debug, PartialEq, Eq)]
enum LinuxServiceState {
    Running,
    InstalledStopped,
    NotInstalled,
}

/// Decide what to report, from whether the unit file exists and what systemd
/// says about it.
///
/// `systemctl --user is-active` alone cannot answer this. Across systemd
/// versions it reports `inactive` for a unit that is merely stopped *and* for
/// one that does not exist, and `unknown` for a missing unit on others. Reading
/// `inactive` as "installed" is what told a user on a machine where the
/// service was never installed to run `systemctl --user start` on nothing.
///
/// So the unit file's presence decides "installed", and `is-active` only
/// decides whether a known-installed service happens to be up. The file is
/// checked on disk rather than via `systemctl is-enabled` because Antra writes
/// it outside systemd's search path, so systemd's own view of the unit is not
/// a reliable proxy for whether install ran.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_service_state(unit_exists: bool, is_active: &str) -> LinuxServiceState {
    if !unit_exists {
        return LinuxServiceState::NotInstalled;
    }
    if is_active == "active" {
        return LinuxServiceState::Running;
    }
    LinuxServiceState::InstalledStopped
}

fn service_status() -> Result<()> {
    println!("{}", "ANTRA SERVICE STATUS".bold());
    println!();

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("launchctl")
            .args(["list", "com.antra.proxy"])
            .output()?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            println!("  {} Service is installed", "✓".green().bold());
            println!();
            for line in stdout.lines() {
                println!("  {}", line.dimmed());
            }
        } else {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is not installed".yellow()
            );
            println!("    Run: antra service install");
        }
    }

    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("systemctl")
            .args(["--user", "is-active", "antra-proxy"])
            .output()?;

        let is_active = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let unit_exists = unit_path().map(|p| p.exists()).unwrap_or(false);

        match linux_service_state(unit_exists, &is_active) {
            LinuxServiceState::Running => {
                println!("  {} Service is running", "✓".green().bold());
            }
            LinuxServiceState::InstalledStopped => {
                println!(
                    "  {} {}",
                    "⚠".yellow().bold(),
                    "Service is installed but not running".yellow()
                );
                println!("    Run: systemctl --user start antra-proxy");
            }
            LinuxServiceState::NotInstalled => {
                println!(
                    "  {} {}",
                    "⚠".yellow().bold(),
                    "Service is not installed".yellow()
                );
                println!("    Run: antra service install");
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("sc.exe")
            .args(["query", "AntraDaemon"])
            .output()?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        if output.status.success() && stdout.contains("AntraDaemon") {
            println!(
                "  {} Service 'AntraDaemon' is installed",
                "✓".green().bold()
            );
            for line in stdout.lines().take(8) {
                println!("  {}", line.trim().dimmed());
            }
        } else {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is not installed".yellow()
            );
            println!("    Run: antra service install (Administrator terminal)");
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Service management is only supported on macOS, Linux, and Windows".red()
        );
    }

    println!();
    Ok(())
}

fn uninstall_service() -> Result<()> {
    println!("{}", "ANTRA SERVICE UNINSTALL".bold());
    println!();

    #[cfg(target_os = "macos")]
    {
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let plist_path = home_dir.join("Library/LaunchAgents/com.antra.proxy.plist");

        if plist_path.exists() {
            // Unload the service
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()])
                .output();

            // Remove the plist file
            std::fs::remove_file(&plist_path)?;

            println!("  {} Service uninstalled", "✓".green().bold());
        } else {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is not installed".yellow()
            );
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Stop and disable the service
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "antra-proxy"])
            .output();

        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "antra-proxy"])
            .output();

        // Remove the service file
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let service_path = home_dir.join(".config/antra/systemd/user/antra-proxy.service");

        if service_path.exists() {
            std::fs::remove_file(&service_path)?;
        }

        println!("  {} Service uninstalled", "✓".green().bold());
    }

    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("sc.exe")
            .args(["stop", "AntraDaemon"])
            .output();
        let output = std::process::Command::new("sc.exe")
            .args(["delete", "AntraDaemon"])
            .output()?;
        if output.status.success() {
            println!("  {} Service 'AntraDaemon' uninstalled", "✓".green().bold());
        } else {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is not installed (or needs an Administrator terminal)".yellow()
            );
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Service management is only supported on macOS, Linux, and Windows".red()
        );
    }

    println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bug this whole refactor exists for: a machine where the service was
    /// never installed, `systemctl --user is-active` reported `inactive` on the
    /// GitHub ubuntu runner, and Antra told the user to
    /// `systemctl --user start` a service that did not exist.
    #[test]
    fn absent_unit_is_never_reported_as_installed() {
        for is_active in [
            "inactive",
            "unknown",
            "active",
            "",
            "activating",
            "deactivating",
            "failed",
        ] {
            assert_eq!(
                linux_service_state(false, is_active),
                LinuxServiceState::NotInstalled,
                "with no unit on disk, is-active={is_active:?} must not imply installed"
            );
        }
    }

    #[test]
    fn present_unit_that_is_active_is_running() {
        assert_eq!(
            linux_service_state(true, "active"),
            LinuxServiceState::Running
        );
    }

    /// A unit that exists but is up for any other reason is installed-and-stopped,
    /// which is the only case where `systemctl --user start` is the right advice.
    #[test]
    fn present_unit_that_is_not_active_is_installed_and_stopped() {
        for is_active in ["inactive", "failed", "activating", "unknown", ""] {
            assert_eq!(
                linux_service_state(true, is_active),
                LinuxServiceState::InstalledStopped,
                "is-active={is_active:?} on an existing unit should mean stopped"
            );
        }
    }

    /// `unit_path` is shared by install and status specifically so they cannot
    /// drift. Assert it points where install writes and nowhere else.
    #[cfg(target_os = "linux")]
    #[test]
    fn unit_path_is_under_the_antra_config_dir() {
        let path = unit_path().expect("home directory should resolve");
        let text = path.to_string_lossy().replace('\\', "/");
        assert!(
            text.contains("/.config/antra/systemd/user/antra-proxy.service"),
            "unexpected unit path: {text}"
        );
    }
}
