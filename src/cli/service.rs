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

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Service install is only supported on macOS and Linux".red()
        );
        Ok(())
    }
}

#[cfg(target_os = "macos")]
fn install_launchd() -> Result<()> {
    use std::path::PathBuf;

    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
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
        println!(
            "  {} Service loaded and enabled",
            "✓".green().bold()
        );
        println!();
        println!("  {}", "Antra proxy will start automatically on login".dimmed());
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
    use std::path::PathBuf;

    let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
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
        println!(
            "  {} Service enabled",
            "✓".green().bold()
        );

        // Start the service
        let start_output = std::process::Command::new("systemctl")
            .args(["--user", "start", "antra-proxy"])
            .output()?;

        if start_output.status.success() {
            println!(
                "  {} Service started",
                "✓".green().bold()
            );
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
    println!("  {}", "Antra proxy will start automatically on login".dimmed());
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

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Service management is only supported on macOS and Linux".red()
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
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let plist_path = home_dir.join("Library/LaunchAgents/com.antra.proxy.plist");

        if plist_path.exists() {
            // Unload the service
            let _ = std::process::Command::new("launchctl")
                .args(["unload", &plist_path.to_string_lossy()])
                .output();

            // Remove the plist file
            std::fs::remove_file(&plist_path)?;

            println!(
                "  {} Service uninstalled",
                "✓".green().bold()
            );
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
        let home_dir = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?;
        let service_path = home_dir.join(".config/antra/systemd/user/antra-proxy.service");

        if service_path.exists() {
            std::fs::remove_file(&service_path)?;
        }

        println!(
            "  {} Service uninstalled",
            "✓".green().bold()
        );
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        println!(
            "  {} {}",
            "✗".red().bold(),
            "Service management is only supported on macOS and Linux".red()
        );
    }

    println!();
    Ok(())
}
