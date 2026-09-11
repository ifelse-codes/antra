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

/// Chown `path` to the invoking (pre-sudo) user when running as root via sudo.
///
/// `sudo antra proxy start` binds :443/:80 as root, but the IPC socket, pid
/// file, log, certs and aliases all live under the user's HOME (sudo
/// preserves HOME on macOS). Left root-owned with mode 0600, the socket is
/// unreachable from the user CLI (`Permission denied`), so `status`/`dev`
/// misreport "not running". Best-effort: logs a warning, never fails.
#[cfg(unix)]
pub fn chown_to_invoking_user(path: &std::path::Path) {
    // Only meaningful as root.
    let euid = unsafe { libc::geteuid() };
    if euid != 0 {
        return;
    }
    let uid: Option<u32> = std::env::var("SUDO_UID").ok().and_then(|s| s.parse().ok());
    let gid: Option<u32> = std::env::var("SUDO_GID").ok().and_then(|s| s.parse().ok());
    let (Some(uid), Some(gid)) = (uid, gid) else {
        return;
    };
    if uid == 0 {
        return;
    }
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    let cstr = match std::ffi::CString::new(bytes) {
        Ok(c) => c,
        Err(_) => return,
    };
    // libc::chown takes uid_t/gid_t (u32 on both macOS and Linux).
    let ret = unsafe { libc::chown(cstr.as_ptr(), uid, gid) };
    if ret != 0 {
        tracing::warn!(
            path = %path.display(),
            uid,
            gid,
            error = %std::io::Error::last_os_error(),
            "Failed to chown to invoking user"
        );
    }
}

/// True when the daemon socket exists but is unreachable due to file
/// permissions — the classic `sudo antra proxy start` (root-owned socket)
/// followed by unprivileged `antra status` shape. Distinguishes "running as
/// root, use sudo" from genuinely not running.
#[cfg(unix)]
pub fn daemon_socket_permission_denied() -> bool {
    let sock = crate::ipc::server::socket_path();
    if !sock.exists() {
        return false;
    }
    match std::os::unix::net::UnixStream::connect(&sock) {
        Ok(_) => false,
        Err(e) => e.kind() == std::io::ErrorKind::PermissionDenied,
    }
}

/// Returns the Windows named pipe path for the daemon IPC.
#[cfg(windows)]
#[allow(dead_code)]
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn chown_helper_is_noop_for_non_root_and_missing_sudo_env() {
        // Never run tests as root in CI; guard anyway so a root run skips
        // instead of chowning temp files.
        let euid = unsafe { libc::geteuid() };
        if euid == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("probe");
        std::fs::write(&p, "x").unwrap();
        // Must not panic, and must leave the file alone.
        chown_to_invoking_user(&p);
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "x");
        // Missing socket path reporting must not panic either.
        let _ = daemon_socket_permission_denied();
    }
}
