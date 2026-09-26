use std::io::Write;
use std::path::Path;

use anyhow::{Context, Result};
use colored::Colorize;

use crate::ipc::client::{is_daemon_running, send_command_sync};
use crate::ipc::protocol::IpcPayload;
use crate::resolver::hosts;
use crate::trust;

pub fn execute(yes: bool) -> Result<()> {
    println!("{}", "ANTRA CLEAN".bold());
    println!();
    println!("  This will permanently remove:");
    println!("    • System trust entries for the current Antra CA");
    println!("    • The complete Antra-managed hosts block");
    println!("    • Root CA certificate and key");
    println!("    • All cached leaf certificates");
    println!("    • Saved static aliases");
    println!("    • Daemon socket and PID file");
    println!();

    if !yes {
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
    print!("  Stopping daemon... ");
    std::io::stdout().flush()?;
    stop_daemon()?;
    println!("{}", "done".green().bold());

    print!("  Removing trusted CA... ");
    std::io::stdout().flush()?;
    trust::remove_ca_noninteractive()
        .context("Trust removal failed; local config and CA were kept for retry")?;
    println!("{}", "done".green().bold());

    print!("  Removing Antra-managed hosts block... ");
    std::io::stdout().flush()?;
    remove_managed_hosts()
        .context("Managed hosts removal failed; local config and CA were kept for retry")?;
    println!("{}", "done".green().bold());

    print!("  Removing runtime state... ");
    std::io::stdout().flush()?;
    remove_runtime_state().context("Failed to remove Antra runtime state")?;
    println!("{}", "done".green().bold());

    print!("  Removing local config and certificates... ");
    std::io::stdout().flush()?;
    remove_local_state().context("Failed to remove Antra local state")?;
    println!("{}", "done".green().bold());

    println!();
    println!("  {}", "All Antra state removed.".green().bold());
    println!();

    Ok(())
}

fn stop_daemon() -> Result<()> {
    if !is_daemon_running() {
        if let Some(pid) = recorded_live_daemon_pid() {
            anyhow::bail!(
                "Daemon (PID {pid}) seems to be running without a socket. Stop it, then retry — refusing to remove state under a live daemon."
            );
        }
        return Ok(());
    }

    send_command_sync(IpcPayload::Shutdown)
        .context("Failed to stop daemon; refusing to remove Antra state")?;
    ensure_daemon_stopped()
}

fn recorded_live_daemon_pid() -> Option<u32> {
    crate::ipc::server::read_daemon_pid().filter(|pid| crate::platform::is_pid_alive(*pid))
}

fn ensure_daemon_stopped() -> Result<()> {
    for _ in 0..50 {
        if recorded_live_daemon_pid().is_none() {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    if let Some(pid) = recorded_live_daemon_pid() {
        anyhow::bail!(
            "Daemon (PID {pid}) is still running. Stop it, then retry — refusing to remove state under a live daemon."
        );
    }
    Ok(())
}

fn remove_managed_hosts() -> Result<()> {
    remove_managed_hosts_at(&hosts::hosts_path())
}

fn remove_managed_hosts_at(path: &Path) -> Result<()> {
    let content = match hosts::read_hosts(path) {
        Ok(content) => content,
        Err(e)
            if e.downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound) =>
        {
            return Ok(())
        }
        Err(e) => return Err(e).with_context(|| format!("Failed to read {}", path.display())),
    };

    let cleaned = hosts::remove_managed_block(&content)
        .with_context(|| format!("Invalid Antra-managed block in {}", path.display()))?;
    if cleaned == content {
        return Ok(());
    }

    hosts::write_hosts_atomic(path, &cleaned)
        .with_context(|| format!("Failed to write {}", path.display()))?;
    let verified =
        hosts::read_hosts(path).with_context(|| format!("Failed to verify {}", path.display()))?;
    let verified_clean = hosts::remove_managed_block(&verified)
        .with_context(|| format!("Invalid Antra-managed block in {}", path.display()))?;
    if verified_clean != verified {
        anyhow::bail!("Antra-managed hosts block remains in {}", path.display());
    }
    Ok(())
}

fn remove_runtime_state() -> Result<()> {
    #[cfg(unix)]
    remove_file_if_exists(&crate::ipc::server::socket_path(), "daemon socket")?;
    remove_file_if_exists(&crate::ipc::server::pid_path(), "daemon PID file")
}

fn remove_local_state() -> Result<()> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?
        .join("antra");
    remove_dir_all_if_exists(&config_dir, "Antra config directory")
}

fn remove_file_if_exists(path: &Path, description: &str) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            Err(e).with_context(|| format!("Failed to remove {description} {}", path.display()))
        }
    }
}

fn remove_dir_all_if_exists(path: &Path, description: &str) -> Result<()> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => {
            Err(e).with_context(|| format!("Failed to remove {description} {}", path.display()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_hosts_cleanup_preserves_unmanaged_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts");
        std::fs::write(
            &path,
            "127.0.0.1 localhost\n# BEGIN ANTRA MANAGED HOSTS\n127.0.0.1 app.test\n# END ANTRA MANAGED HOSTS\n# keep\n",
        )
        .unwrap();

        remove_managed_hosts_at(&path).unwrap();

        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            "127.0.0.1 localhost\n# keep\n"
        );
    }

    #[test]
    fn managed_hosts_cleanup_missing_file_is_success() {
        let dir = tempfile::tempdir().unwrap();
        assert!(remove_managed_hosts_at(&dir.path().join("hosts")).is_ok());
    }

    #[test]
    fn managed_hosts_cleanup_rejects_malformed_block_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hosts");
        let content = "# END ANTRA MANAGED HOSTS\n# BEGIN ANTRA MANAGED HOSTS\n";
        std::fs::write(&path, content).unwrap();

        assert!(remove_managed_hosts_at(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
}
