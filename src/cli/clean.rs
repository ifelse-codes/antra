use std::io::Write;

use anyhow::Result;
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;

pub fn execute(yes: bool) -> Result<()> {
    println!("{}", "ANTRA CLEAN".bold());
    println!();
    println!("  This will remove:");
    println!("    • Root CA certificate and key");
    println!("    • All cached leaf certificates");
    println!("    • Daemon socket and PID file");
    println!();

    if !yes {
        // Confirmation prompt
        print!("  Continue? [y/N] ");
        std::io::stdout().flush()?;

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim().to_lowercase();

        if input != "y" && input != "yes" {
            println!();
            println!("  {}", "Cancelled.".dimmed());
            return Ok(());
        }
    }

    println!();

    // Stop daemon first if running
    if is_daemon_running() {
        print!("  Stopping daemon... ");
        match send_command_sync(IpcPayload::Shutdown) {
            Ok(_) => {
                println!("{}", "✓".green().bold());
                // Wait for daemon to exit
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            Err(_) => {
                println!("{}", "✓".green().bold());
                // Force cleanup even if IPC fails
                #[cfg(unix)]
                {
                    let sock = crate::ipc::server::socket_path();
                    let _ = std::fs::remove_file(&sock);
                }
            }
        }
    }

    // Remove daemon files
    print!("  Removing daemon files... ");
    #[cfg(unix)]
    {
        let sock = crate::ipc::server::socket_path();
        let _ = std::fs::remove_file(&sock);
    }
    let pid = crate::ipc::server::pid_path();
    let _ = std::fs::remove_file(&pid);
    println!("{}", "✓".green().bold());

    // Remove certificates
    print!("  Removing certificates... ");
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("antra");

    if config_dir.exists() {
        std::fs::remove_dir_all(&config_dir)?;
    }
    println!("{}", "✓".green().bold());

    println!();
    println!("  {}", "All Antra state removed.".green().bold());
    println!();
    Ok(())
}
