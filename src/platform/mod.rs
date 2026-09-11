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

/// True when a PID refers to a live process.
///
/// Unix: signal-0 probe. Windows: `tasklist /FI "PID eq <pid>"` — avoids new
/// native deps and works for any process the caller can see. Used by route
/// restore (drop stale managed routes) and `--force` kill paths. Infrequent
/// calls only; never on the proxy hot path.
#[cfg(unix)]
pub fn is_pid_alive(pid: u32) -> bool {
    // nix is only a unix dependency.
    #[allow(clippy::useless_conversion)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        kill(Pid::from_raw(pid as i32), None).is_ok()
    }
}

/// Windows PID liveness via `tasklist`.
#[cfg(windows)]
pub fn is_pid_alive(pid: u32) -> bool {
    let Ok(output) = std::process::Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
    else {
        // If we cannot query, assume alive so callers don't delete routes
        // or skip kills based on a probe failure.
        return true;
    };
    if !output.status.success() {
        return true;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .any(|line| tasklist_line_matches_pid(line, pid))
}

/// True when a `tasklist /FO CSV /NH` line reports `pid` in its PID column.
///
/// CSV lines look like: `"node.exe","1234","Console","1","45,000 K"`.
/// Only the 2nd field (PID) is compared: matching any field false-positives
/// on the session column (`"1"`) and on mem-usage fragments (`"45,000 K"`
/// splits at the comma, yielding a bare `"45` fragment). Splitting on `","`
/// (rather than `,`) keeps commas inside quoted fields from shifting columns.
/// Pure parsing, compiled everywhere under `test` so CI covers it.
#[cfg(any(test, windows))]
fn tasklist_line_matches_pid(line: &str, pid: u32) -> bool {
    let trimmed = line.trim();
    let inner = trimmed
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(trimmed);
    inner
        .split("\",\"")
        .nth(1)
        .is_some_and(|field| field == pid.to_string())
}

#[cfg(test)]
mod tasklist_tests {
    use super::*;

    #[test]
    fn pid_column_matches() {
        assert!(tasklist_line_matches_pid(
            r#""node.exe","1234","Console","1","45,000 K""#,
            1234
        ));
    }

    #[test]
    fn session_column_does_not_match() {
        // Session id "1" must not count as PID 1.
        assert!(!tasklist_line_matches_pid(
            r#""node.exe","1234","Console","1","45,000 K""#,
            1
        ));
    }

    #[test]
    fn memory_fragment_does_not_match() {
        // "45,000 K" splits at the comma — the "45 fragment is not a PID.
        assert!(!tasklist_line_matches_pid(
            r#""node.exe","1234","Console","1","45,000 K""#,
            45
        ));
        assert!(!tasklist_line_matches_pid(
            r#""node.exe","1234","Console","1","45,000 K""#,
            45_000
        ));
    }

    #[test]
    fn image_name_and_header_lines_do_not_match() {
        assert!(!tasklist_line_matches_pid(
            r#""1234.exe","5678","Console","1","8,000 K""#,
            1234
        ));
        assert!(!tasklist_line_matches_pid(
            r#"INFO: No tasks are running which match the specified criteria."#,
            1234
        ));
        assert!(!tasklist_line_matches_pid("", 1234));
    }
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
