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
    let log_path = crate::util::logs::daemon_log_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Carry the documented port overrides into the unit, so the service binds
    // what the installing user's CLI would. The user manager does not inherit
    // this shell's environment. Only real port numbers: the value lands on an
    // `Environment=` line, where whitespace or a newline would corrupt it.
    let env: Vec<(&str, String)> = ["ANTRA_PORT", "ANTRA_HTTP_PORT"]
        .into_iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k, v)))
        .filter(|(_, v)| v.parse::<u16>().is_ok())
        .collect();

    // A re-install that fails must leave the working unit it found, not
    // delete it.
    let previous = std::fs::read(&service_path).ok();
    std::fs::create_dir_all(&service_dir)?;
    std::fs::write(&service_path, systemd_unit(&antra_path, &log_path, &env))?;

    // systemd only sees a new unit file after a reload. Without this, enable
    // fails with "Unit file antra-proxy.service does not exist". A failure
    // here or at enable rolls the file back, so a failed install changes
    // nothing on disk and "installed" (the file's presence, see
    // `linux_service_state`) never outlives one.
    for (step, args) in [
        ("reload systemd", &["daemon-reload"][..]),
        ("enable the service", &["enable", "antra-proxy"][..]),
    ] {
        if let Err(reason) = systemctl_user(args) {
            let _ = match &previous {
                Some(bytes) => std::fs::write(&service_path, bytes),
                None => std::fs::remove_file(&service_path),
            };
            let _ = systemctl_user(&["daemon-reload"]);
            println!("  {} Failed to {step}: {reason}", "✗".red().bold());
            print_no_user_systemd_hint(&reason);
            println!();
            return Ok(());
        }
    }
    println!(
        "  {} Created systemd user unit: {}",
        "✓".green().bold(),
        service_path.display()
    );

    // v0.6.1 and earlier wrote the unit where systemd never looked, so it was
    // never loaded or enabled and holds no state worth keeping. Remove it so
    // there is one unit on disk, not two.
    if let Ok(legacy) = legacy_unit_path() {
        if legacy.exists() && std::fs::remove_file(&legacy).is_ok() {
            println!(
                "  {} Removed the unit an older Antra left at {}",
                "✓".green().bold(),
                legacy.display()
            );
        }
    }
    println!("  {} Service enabled", "✓".green().bold());

    if systemctl_stdout(&["is-active", "antra-proxy"]) == "active" {
        // A re-install: the service is already up on the previous unit.
        println!("  {} Service is already running", "✓".green().bold());
        println!(
            "    {}",
            "To apply the new unit: systemctl --user restart antra-proxy".dimmed()
        );
    } else if daemon_running_outside_service() {
        // Starting now would put a second daemon on the same socket and
        // ports; it would exit, and `Restart=always` would retry it every few
        // seconds until the first one idled out. Stopping the running one
        // would drop the routes of every `antra run` using it. Leave it, and
        // say how to hand over.
        println!(
            "  {} {}",
            "⚠".yellow().bold(),
            "A daemon is already running outside the service, so the service was not started now."
                .yellow()
        );
        println!("    It will start at your next login. To hand over now:");
        println!("    antra proxy stop && systemctl --user start antra-proxy");
        println!();
        return Ok(());
    } else {
        if let Err(reason) = systemctl_user(&["start", "antra-proxy"]) {
            println!(
                "  {} Service enabled but failed to start: {reason}",
                "✗".red().bold()
            );
            println!("    Check: systemctl --user status antra-proxy");
            println!();
            return Ok(());
        }
        // `Type=simple` reports success the moment the process is forked,
        // so a daemon that dies on a port conflict would still read as
        // started. Wait for the IPC socket, as `ensure_daemon` does.
        let started = (0..30).any(|_| {
            std::thread::sleep(std::time::Duration::from_millis(100));
            crate::ipc::client::is_daemon_running()
        });
        if !started {
            println!(
                "  {} Service started but the daemon did not come up",
                "✗".red().bold()
            );
            println!("    Check: antra logs   or   systemctl --user status antra-proxy");
            println!();
            return Ok(());
        }
        println!("  {} Service started", "✓".green().bold());
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

/// The systemd user unit `antra service install` writes.
///
/// `ANTRA_DAEMON=1` is what makes this work at all. Without it,
/// `antra proxy start` forks the daemon into the background and exits; under
/// `Type=simple` systemd reads the exit as the service stopping, kills the
/// rest of the unit's cgroup — the daemon — and `Restart=always` starts the
/// cycle again five seconds later. With it, `proxy start` runs the daemon in
/// the foreground, exactly as the CLI's own auto-start does.
///
/// Output goes to the same file `antra logs` reads, as the launchd plist
/// does. systemd older than 240 ignores `append:` with a warning and falls
/// back to the journal, which loses `antra logs` but not the service.
///
/// Not `#[cfg(target_os = "linux")]`, for the reason given on
/// `LinuxServiceState`: pure string logic should be tested everywhere.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn systemd_unit(exe: &std::path::Path, log: &std::path::Path, env: &[(&str, String)]) -> String {
    let mut environment = String::from("Environment=ANTRA_DAEMON=1\n");
    for (key, value) in env {
        environment.push_str(&format!("Environment={key}={value}\n"));
    }
    format!(
        r#"[Unit]
Description=Antra Local Development Proxy
After=network.target

[Service]
Type=simple
ExecStart={exe} proxy start
{environment}StandardOutput=append:{log}
StandardError=append:{log}
Restart=always
RestartSec=5

[Install]
WantedBy=default.target
"#,
        exe = exe.display(),
        log = log.display(),
    )
}

/// Run `systemctl --user <args>`, returning why it failed if it did.
///
/// A missing `systemctl` (a distro without systemd, WSL1) is reported in
/// words rather than as a bare `No such file or directory (os error 2)`.
#[cfg(target_os = "linux")]
fn systemctl_user(args: &[&str]) -> std::result::Result<(), String> {
    match std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                format!("systemctl exited with {}", output.status)
            } else {
                stderr
            })
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Err("systemctl not found — this system does not use systemd".to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// `systemctl --user <args>` stdout, trimmed; empty if it could not run.
#[cfg(target_os = "linux")]
fn systemctl_stdout(args: &[&str]) -> String {
    std::process::Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

/// When `systemctl --user` cannot reach a user manager, say where that
/// happens and what still works, instead of leaving a D-Bus error alone.
#[cfg(target_os = "linux")]
fn print_no_user_systemd_hint(reason: &str) {
    if reason.starts_with("systemctl not found") || reason.contains(" bus") {
        println!(
            "    {}",
            "No systemd user session here (common over SSH without lingering, in WSL, or in containers)."
                .dimmed()
        );
        println!(
            "    {}",
            "Antra still works: the daemon starts on demand, or run `antra proxy start`.".dimmed()
        );
    }
}

/// A daemon the service did not start: one the CLI auto-started, or a
/// root-owned one from `sudo antra proxy start`.
#[cfg(target_os = "linux")]
fn daemon_running_outside_service() -> bool {
    crate::ipc::client::is_daemon_running() || crate::platform::daemon_socket_permission_denied()
}

/// Where `antra service install` puts the systemd user unit, and where
/// `antra service status` looks for it.
///
/// Shared so the two cannot drift. `$XDG_CONFIG_HOME/systemd/user` (else
/// `~/.config/systemd/user`) is the one per-user directory on systemd's
/// search path that is meant for units a user writes.
#[cfg(target_os = "linux")]
fn unit_path() -> Result<std::path::PathBuf> {
    let config_dir =
        dirs::config_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?;
    Ok(config_dir.join("systemd/user").join("antra-proxy.service"))
}

/// Where v0.6.1 and earlier wrote the unit: `~/.config/antra/systemd/user/`.
///
/// Not on systemd's search path, which is ROADMAP C14 — install always
/// failed at `enable`, so a file here was never loaded and does not mean
/// "installed". Install and uninstall remove it; status only mentions it.
#[cfg(target_os = "linux")]
fn legacy_unit_path() -> Result<std::path::PathBuf> {
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
/// decides whether a known-installed service happens to be up. Install rolls
/// the file back when systemd refuses it, so a file on disk is a unit systemd
/// accepted, and checking it needs no user session bus.
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
                if legacy_unit_path().map(|p| p.exists()).unwrap_or(false) {
                    println!(
                        "    {}",
                        "An older Antra wrote a unit systemd never loaded; install replaces it."
                            .dimmed()
                    );
                }
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
        let service_path = unit_path()?;
        let legacy_path = legacy_unit_path()?;

        if service_path.exists() {
            // Stop and disable while systemd can still see the unit, then
            // remove it and reload so systemd forgets it too.
            let _ = systemctl_user(&["stop", "antra-proxy"]);
            let _ = systemctl_user(&["disable", "antra-proxy"]);
            std::fs::remove_file(&service_path)?;
            let _ = systemctl_user(&["daemon-reload"]);
            println!("  {} Service uninstalled", "✓".green().bold());
        } else {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is not installed".yellow()
            );
        }

        // Left by v0.6.1 and earlier; never loaded, so only the file goes.
        if legacy_path.exists() {
            std::fs::remove_file(&legacy_path)?;
            println!(
                "  {} Removed the unit an older Antra left at {}",
                "✓".green().bold(),
                legacy_path.display()
            );
        }
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

    /// ROADMAP C14: the unit must be where `systemctl --user` looks —
    /// `$XDG_CONFIG_HOME/systemd/user` — and not under Antra's own config
    /// dir, which systemd never searches.
    #[cfg(target_os = "linux")]
    #[test]
    fn unit_path_is_on_systemds_user_search_path() {
        let path = unit_path().expect("config directory should resolve");
        let expected = dirs::config_dir()
            .expect("config directory should resolve")
            .join("systemd/user/antra-proxy.service");
        assert_eq!(path, expected);
        assert_ne!(
            path,
            legacy_unit_path().expect("home directory should resolve"),
            "install must not write where v0.6.1 did"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn legacy_unit_path_is_where_v0_6_1_wrote() {
        let path = legacy_unit_path().expect("home directory should resolve");
        let text = path.to_string_lossy();
        assert!(
            text.ends_with("/.config/antra/systemd/user/antra-proxy.service"),
            "unexpected legacy path: {text}"
        );
    }

    fn unit_for(env: &[(&str, String)]) -> String {
        systemd_unit(
            std::path::Path::new("/opt/antra/bin/antra"),
            std::path::Path::new("/home/u/.local/share/antra/daemon.log"),
            env,
        )
    }

    /// Without `ANTRA_DAEMON=1`, `proxy start` forks and exits, systemd kills
    /// the forked daemon with the rest of the cgroup, and `Restart=always`
    /// loops. The foreground run is the whole fix; pin it.
    #[test]
    fn unit_runs_the_daemon_in_the_foreground() {
        let unit = unit_for(&[]);
        assert!(unit.contains("\nType=simple\n"), "{unit}");
        assert!(
            unit.contains("\nExecStart=/opt/antra/bin/antra proxy start\n"),
            "{unit}"
        );
        assert!(unit.contains("\nEnvironment=ANTRA_DAEMON=1\n"), "{unit}");
    }

    #[test]
    fn unit_logs_where_antra_logs_reads() {
        let unit = unit_for(&[]);
        assert!(
            unit.contains("\nStandardOutput=append:/home/u/.local/share/antra/daemon.log\n"),
            "{unit}"
        );
        assert!(
            unit.contains("\nStandardError=append:/home/u/.local/share/antra/daemon.log\n"),
            "{unit}"
        );
    }

    #[test]
    fn unit_carries_port_overrides_only_when_set() {
        let unit = unit_for(&[]);
        assert!(!unit.contains("ANTRA_PORT"), "{unit}");
        assert!(!unit.contains("ANTRA_HTTP_PORT"), "{unit}");

        let unit = unit_for(&[
            ("ANTRA_PORT", "18443".to_string()),
            ("ANTRA_HTTP_PORT", "18080".to_string()),
        ]);
        assert!(unit.contains("\nEnvironment=ANTRA_PORT=18443\n"), "{unit}");
        assert!(
            unit.contains("\nEnvironment=ANTRA_HTTP_PORT=18080\n"),
            "{unit}"
        );
    }

    /// Every line systemd would reject silently is a line that does nothing.
    /// Each non-blank, non-section line must be `Key=value` with a key from
    /// the section it sits in.
    #[test]
    fn unit_lines_are_well_formed() {
        let unit = unit_for(&[("ANTRA_PORT", "18443".to_string())]);
        let mut section = "";
        for line in unit.lines().filter(|l| !l.is_empty()) {
            if line.starts_with('[') {
                section = line;
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("not Key=value: {line:?}"));
            assert!(!value.is_empty(), "empty value: {line:?}");
            let allowed: &[&str] = match section {
                "[Unit]" => &["Description", "After"],
                "[Service]" => &[
                    "Type",
                    "ExecStart",
                    "Environment",
                    "StandardOutput",
                    "StandardError",
                    "Restart",
                    "RestartSec",
                ],
                "[Install]" => &["WantedBy"],
                other => panic!("unexpected section {other:?}"),
            };
            assert!(allowed.contains(&key), "{key} is not a {section} key");
        }
    }
}
