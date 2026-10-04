use std::path::PathBuf;

#[cfg(unix)]
pub mod sudo;

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

/// The uid of the user this process is acting for: the pre-`sudo` user when
/// running as root, the process's own euid otherwise.
///
/// `None` when the process is root but `SUDO_UID` is absent or unusable —
/// there is no invoking user to hand anything back to, and guessing (root)
/// would be wrong.
#[cfg(unix)]
pub(crate) fn acting_uid() -> Option<u32> {
    let euid = unsafe { libc::geteuid() };
    if euid != 0 {
        return Some(euid);
    }
    std::env::var("SUDO_UID")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&uid| uid != 0)
}

/// Whether this process may tighten a directory owned by `dir_uid`.
///
/// Only directories belonging to our own euid, or — as root — to the user we
/// are acting for. Anything else is a pre-existing directory we did not
/// create and cannot honestly claim: leave it alone and report it rather
/// than silently trusting (or silently reowning) someone else's tree.
#[cfg(unix)]
fn may_tighten_mode(dir_uid: u32, euid: u32, acting_uid: u32) -> bool {
    dir_uid == euid || dir_uid == acting_uid
}

/// Create `dir` — and any parents it is missing — as private `0o700`
/// directories, then hand every component this call created back to the
/// invoking user under `sudo`. Unix only.
///
/// The daemon's control socket is a Unix socket, and both halves of its
/// safety depend on the *directory* rather than on the socket file:
///
/// * **The bind-then-chmod window.** A socket file is created by `bind(2)`
///   at the process umask (`0o755` under the usual `0o022`), so between the
///   bind and the `chmod 0o600` that follows it, any local user can connect
///   and issue IPC commands. `connect` needs `x` on *every* component of the
///   path, so a socket that is briefly `0o755` inside a `0o700` directory is
///   still unreachable by anyone else for the whole of that window. The
///   directory has to be private *before* the bind — tightening it afterwards
///   would just move the race.
/// * **The `/tmp` fallback layout.** When the derived socket path does not fit
///   `sun_path`, the daemon falls back to `/tmp/antra-<uid>/<hash>/d.sock`
///   (see `ipc::server::socket_path`). `create_dir_all` under the default
///   umask leaves that at `0o755`, so anyone can list the per-home hashes.
///
/// **Chowning is not optional.** `sudo antra proxy start` runs this as root
/// with a user-owned `HOME`, and a root-owned `0o700` directory is
/// unsearchable for that user — which locks the unprivileged CLI out of its
/// own daemon, exactly the failure `chown_to_invoking_user` exists to prevent.
/// Every component *this call created* is chowned, not only the leaf:
/// tightening the leaf alone would leave an unsearchable root-owned ancestor
/// (`/tmp/antra-<uid>`) above it and break the same flow.
///
/// Best-effort on tightening, strict on creation: a component that cannot be
/// chmod-ed or chowned is reported and skipped, so a read-only or foreign
/// directory degrades to the previous behaviour instead of failing a start.
#[cfg(unix)]
pub fn ensure_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};

    // Work out which components are missing before creating them:
    // `create_dir_all` does not report what it made, and the chown below has
    // to cover every one of them. Leaf-first, so the walk stops at the first
    // component that already exists.
    let mut missing: Vec<&std::path::Path> = Vec::new();
    let mut cursor = dir;
    loop {
        match std::fs::symlink_metadata(cursor) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        missing.push(cursor);
        match cursor.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => cursor = parent,
            _ => break,
        }
    }

    // `mode` applies at creation only, and the umask still masks it — no umask
    // in practical use clears any of `0o700`'s three bits, and the explicit
    // chmod below makes the result independent of the umask anyway.
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;

    let euid = unsafe { libc::geteuid() };
    let acting_uid = acting_uid().unwrap_or(euid);
    for path in missing.iter().rev().copied().chain(std::iter::once(dir)) {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            continue;
        };
        if !may_tighten_mode(meta.uid(), euid, acting_uid) {
            tracing::warn!(
                path = %path.display(),
                owner_uid = meta.uid(),
                "Leaving a directory not owned by the invoking user alone (its mode is not tightened)"
            );
            continue;
        }
        if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "Failed to set private mode on directory"
            );
            continue;
        }
        chown_to_invoking_user(path);
    }
    Ok(())
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
/// restore and the daemon's reaper (drop stale managed routes) and `--force`
/// kill paths. Infrequent calls only; never on the proxy hot path.
#[cfg(unix)]
pub fn is_pid_alive(pid: u32) -> bool {
    // `kill(0, …)` and a negative pid address process *groups*, so neither
    // can say anything about one process — and reading a group as "alive"
    // would keep a dead route forever.
    let Ok(raw) = i32::try_from(pid) else {
        return false;
    };
    if raw == 0 {
        return false;
    }
    // nix is only a unix dependency.
    #[allow(clippy::useless_conversion)]
    {
        use nix::errno::Errno;
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        // EPERM means the process exists and belongs to someone else — a
        // `sudo antra run` child seen from a user-level daemon. Calling it
        // dead would let the reaper delete a live route.
        if !matches!(kill(Pid::from_raw(raw), None), Ok(()) | Err(Errno::EPERM)) {
            return false;
        }
    }
    // A zombie still answers signal 0 until its parent reaps it. The child
    // of a SIGKILLed `antra run` is reparented, and a container's init can
    // take seconds to reap it — long enough to call a dead server running.
    #[cfg(target_os = "linux")]
    if let Ok(stat) = std::fs::read_to_string(format!("/proc/{raw}/stat")) {
        return !matches!(proc_stat_state(&stat), Some('Z' | 'X'));
    }
    true
}

/// The state letter in a `/proc/<pid>/stat` line (`R`, `S`, `Z`, …). The
/// command name before it is parenthesised free text, so count from the
/// last `)`.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn proc_stat_state(stat: &str) -> Option<char> {
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .next()?
        .chars()
        .next()
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

    use std::os::unix::fs::PermissionsExt;

    fn mode_of(path: &std::path::Path) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    /// Run `f` with a fully permissive umask.
    ///
    /// The umask is process-global, so this is only sound because it is
    /// restored before returning and because nothing else in this test binary
    /// asserts on a directory mode it did not set itself. It exists so the
    /// assertion below is about the code under test rather than about the
    /// umask the runner happened to have: with `0o022` (or worse, `0o077`) a
    /// `create_dir_all` could pass by accident.
    fn with_permissive_umask<T>(f: impl FnOnce() -> T) -> T {
        // SAFETY: `umask` only reads/writes the process umask word; both
        // calls happen on this thread with no filesystem work in between.
        let previous = unsafe { libc::umask(0) };
        let out = f();
        unsafe { libc::umask(previous) };
        out
    }

    /// The `/tmp/antra-<uid>/<hash>/` fallback layout must come out private.
    /// `create_dir_all` under the default umask leaves it `0o755`, which lets
    /// any local user enumerate the per-home hashes.
    #[test]
    fn ensure_private_dir_creates_the_whole_chain_0700() {
        let tmp = tempfile::tempdir().unwrap();
        let per_uid = tmp.path().join("antra-501");
        let hashed = per_uid.join("3f9a1c7e");
        let dir = hashed.join("nested");

        with_permissive_umask(|| ensure_private_dir(&dir).unwrap());

        for created in [&per_uid, &hashed, &dir] {
            assert_eq!(
                mode_of(created),
                0o700,
                "{} is not private",
                created.display()
            );
        }
    }

    /// An already-existing directory (every daemon restart) is tightened too,
    /// not just one this call happened to create — otherwise a `0o755`
    /// directory from an older install would keep the socket reachable at its
    /// bind-time mode forever.
    #[test]
    fn ensure_private_dir_tightens_a_preexisting_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("antra");
        std::fs::create_dir(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            mode_of(&dir),
            0o755,
            "precondition: an old world-readable dir"
        );

        ensure_private_dir(&dir).unwrap();

        assert_eq!(mode_of(&dir), 0o700);
    }

    /// Only our own directory — or, as root, the one belonging to the user we
    /// act for — may be tightened. A pre-existing directory owned by somebody
    /// else is left exactly as it is, and reported.
    #[test]
    fn may_tighten_only_our_own_or_the_invoking_users_directory() {
        // euid, acting uid, foreign uid
        assert!(may_tighten_mode(501, 501, 501));
        // A root daemon (euid 0) may tighten a directory the invoking user owns.
        assert!(may_tighten_mode(501, 0, 501));
        // But not one belonging to an unrelated user.
        assert!(!may_tighten_mode(502, 0, 501));
        assert!(!may_tighten_mode(502, 501, 501));
    }

    /// A missing directory that cannot be created is an error, not a silent
    /// success: the caller binds into this path immediately afterwards, and a
    /// bind that fails later reads like a product bug.
    #[test]
    fn ensure_private_dir_propagates_creation_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let blocker = tmp.path().join("not-a-dir");
        std::fs::write(&blocker, "x").unwrap();
        let err = ensure_private_dir(&blocker.join("child")).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotADirectory);
    }

    #[test]
    fn is_pid_alive_tells_live_from_dead() {
        assert!(is_pid_alive(std::process::id()));
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        // Reaped, so the pid is free. (It could be reused, but not this fast
        // on any kernel that hands pids out sequentially.)
        assert!(!is_pid_alive(pid));
    }

    /// An exited child its parent has not reaped yet is a zombie: signal 0
    /// still succeeds, but nothing is running.
    #[cfg(target_os = "linux")]
    #[test]
    fn is_pid_alive_calls_a_zombie_dead() {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        // Not reaped yet: wait for it to become a zombie, then ask.
        let stat = format!("/proc/{pid}/stat");
        for _ in 0..200 {
            let state = std::fs::read_to_string(&stat)
                .ok()
                .and_then(|s| proc_stat_state(&s));
            if state == Some('Z') {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!is_pid_alive(pid), "zombie {pid} reported alive");
        child.wait().unwrap();
    }

    #[test]
    fn proc_stat_state_reads_past_a_tricky_command_name() {
        assert_eq!(proc_stat_state("8429 (sleep) Z 8427 8427"), Some('Z'));
        assert_eq!(proc_stat_state("77 (node a) b) S 70 75"), Some('S'));
        assert_eq!(proc_stat_state(""), None);
    }

    /// pid 1 always exists and, for anyone but root, answers signal 0 with
    /// EPERM. That is "alive, not yours" — and the reaper must not delete a
    /// `sudo antra run` route because of it. CI runs unprivileged, so this
    /// is where the EPERM branch is exercised.
    #[test]
    fn is_pid_alive_counts_another_users_process_as_alive() {
        assert!(is_pid_alive(1));
    }

    /// 0 and pids past `i32::MAX` address process groups in `kill(2)`, not
    /// a process; reading one as alive would pin a dead route forever.
    #[test]
    fn is_pid_alive_rejects_group_addresses() {
        assert!(!is_pid_alive(0));
        assert!(!is_pid_alive(u32::MAX));
    }
}
