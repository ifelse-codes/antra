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
    println!("    • Saved static aliases");
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
                // IPC failed — only force-clean when the recorded daemon is
                // actually dead. SIGTERM a live-but-unresponsive daemon and
                // wait; never delete the socket of a process we couldn't stop
                // (that orphans it: ports held, invisible, next start steals
                // the socket).
                #[cfg(unix)]
                if let Some(pid) = crate::ipc::server::is_daemon_pid_alive() {
                    println!("{}", "unresponsive, signalling...".yellow().bold());
                    signal_pid(pid);
                    let mut waited = 0;
                    while crate::ipc::server::is_daemon_pid_alive().is_some() && waited < 20 {
                        std::thread::sleep(std::time::Duration::from_millis(100));
                        waited += 1;
                    }
                    if crate::ipc::server::is_daemon_pid_alive().is_some() {
                        anyhow::bail!(
                            "Daemon (PID {pid}) is running but will not stop. Stop it with `kill {pid}`, then retry — refusing to wipe state under a live daemon."
                        );
                    }
                }
                println!("{}", "✓".green().bold());
            }
        }
    } else {
        // No reachable socket — still refuse to wipe under a live daemon
        // that merely lost its socket file.
        #[cfg(unix)]
        if let Some(pid) = crate::ipc::server::is_daemon_pid_alive() {
            anyhow::bail!(
                "Daemon (PID {pid}) seems to be running without a socket. Stop it with `kill {pid}`, then retry — refusing to wipe state under a live daemon."
            );
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
        let _ = std::fs::remove_dir_all(&config_dir);
    }
    println!("{}", "✓".green().bold());

    println!();
    println!("  {}", "All Antra state removed.".green().bold());
    println!();

    Ok(())
}

/// Best-effort graceful shutdown of a PID (unix). The daemon handles
/// SIGTERM by unregistering nothing (managed routes die with it) and
/// removing its own socket + pid file.
#[cfg(unix)]
fn signal_pid(pid: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
}
