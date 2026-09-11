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
    <string>{}/.config/antra/daemon.log</string>

    <key>StandardErrorPath</key>
    <string>{}/.config/antra/daemon.log</string>
</dict>
</plist>"#,
        antra_path.display(),
        home_dir.display(),
        home_dir.display()
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
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
    let config_dir = home_dir.join(".config/antra");
    let service_dir = config_dir.join("systemd/user");
    let service_path = service_dir.join("antra-proxy.service");
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

        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if status == "active" {
            println!("  {} Service is running", "✓".green().bold());
        } else if status == "inactive" {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is installed but not running".yellow()
            );
            println!("    Run: systemctl --user start antra-proxy");
        } else {
            println!(
                "  {} {}",
                "⚠".yellow().bold(),
                "Service is not installed".yellow()
            );
            println!("    Run: antra service install");
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
