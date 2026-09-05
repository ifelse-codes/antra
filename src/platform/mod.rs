use std::path::PathBuf;

/// Returns the IPC socket/pipe path for the daemon.
#[cfg(unix)]
#[allow(dead_code)]
pub fn ipc_path() -> PathBuf {
    let dir = dirs::runtime_dir()
        .or_else(dirs::data_local_dir)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    dir.join("antra").join("daemon.sock")
}

/// Returns the IPC socket/pipe path for the daemon.
#[cfg(windows)]
#[allow(dead_code)]
pub fn ipc_path() -> PathBuf {
    // Windows named pipes use a special path format, but we store
    // the PID file in a regular directory
    let dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
    dir.join("antra")
}

/// Returns the path to the daemon PID file.
#[allow(dead_code)]
pub fn pid_file_path() -> PathBuf {
    #[cfg(unix)]
    {
        let dir = dirs::runtime_dir()
            .or_else(dirs::data_local_dir)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        dir.join("antra").join("daemon.pid")
    }
    #[cfg(windows)]
    {
        let dir = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\ProgramData"));
        dir.join("antra").join("daemon.pid")
    }
}

/// Returns the Antra config directory (~/.config/antra/).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("antra")
}

/// Set restrictive permissions on a key file (0o600 on Unix, no-op on Windows).
#[allow(dead_code)]
pub fn set_key_permissions(path: &std::path::Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    let _ = path;
    Ok(())
}

/// Returns the Windows named pipe path for the daemon IPC.
#[cfg(windows)]
pub fn named_pipe_path() -> String {
    r"\\.\pipe\antra-daemon".to_string()
}

// Platform-specific modules for future use
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "windows")]
pub mod windows;
