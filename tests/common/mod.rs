//! Shared plumbing for the end-to-end suites (`e2e_binary`, `e2e_adversarial`).
//!
//! Every `antra` process these suites spawn runs against a **disposable
//! HOME**, never the developer's. That used to be a manual shell wrapper
//! (`HOME=$(mktemp -d) cargo test`, `AGENT.md`) and the gap already bit
//! someone: `cargo test` once regenerated the real local CA mid-session
//! (`tests/user-test-2026-09-06-021.md`). Once the CA can rotate itself
//! (`CA_VERSION` marker in `certs/store.rs`), an unhermetic suite would
//! silently rotate the developer's CA and desync their keychain, so the
//! redirect now lives in code rather than in a README.
//!
//! The env map covers every path `dirs` can resolve for the code under test
//! (`platform::config_dir`, `platform::ipc_path`): config, data-local, and
//! runtime/socket dirs on unix; roaming + local appdata on Windows.
//!
//! `TestHome::shared()` is deliberate: both e2e binaries run their own
//! `antra` processes against a *shared* disposable home, so they share one
//! daemon. The daemon has a single fallback port (443 → 8443, `daemon/server.rs`),
//! so a second home would mean a second daemon that cannot bind and bails with
//! "Cannot start daemon: HTTPS unavailable". The shared home is derived from
//! the user name so the two binaries (which `cargo test` runs sequentially)
//! land on the same socket; it is not cleaned up automatically, so it is
//! created under the system temp dir with a stable, obviously-disposable name.

// Every test binary compiles this module and uses a subset of it — the TLS
// suite spawns without a working directory, the CLI suites do. A module-level
// allow describes that honestly; annotating each helper per binary would be
// noise that hides real dead code elsewhere.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use tempfile::TempDir;

/// Root under which every disposable home lives.
///
/// Deliberately `/tmp` rather than `std::env::temp_dir()`: on macOS the
/// per-user temp root is already ~50 characters deep, and the daemon's socket
/// falls back to `$HOME/Library/Application Support/antra/daemon.sock`
/// because `dirs::runtime_dir()` is `None` there — so a home under
/// `$TMPDIR` produced a 110-character path and the daemon refused to start
/// with `path must be shorter than SUN_LEN`. `SUN_PATH_MAX` is 104.
fn base_root() -> PathBuf {
    let root = if cfg!(windows) {
        std::env::temp_dir().join("antra-e2e")
    } else {
        PathBuf::from("/tmp").join("antra-e2e")
    };
    std::fs::create_dir_all(&root).expect("test root must be creatable");
    root
}

/// The one home shared by every test in a binary. See module docs: the
/// daemon has a single fallback port (443 → 8443, `daemon/server.rs`), so a
/// second home per suite would mean a second daemon that cannot bind and
/// bails. The user name keeps the path stable across the e2e binaries,
/// which `cargo test` runs sequentially.
static SHARED_HOME: LazyLock<TestHome> = LazyLock::new(|| {
    let path = base_root().join(user_tag());
    create_layout(&path);
    // The suites start a real daemon (that is the point of the e2e ones) and
    // a daemon outlives the process that spawned it. Left running, it would
    // hold 8443 after `cargo test` and the developer's next `antra run` would
    // bail with "HTTPS unavailable (Both 443 and 8443 are in use)". Rust has
    // no exit hook, so register the C one: it runs on normal process exit,
    // which is exactly when we want the daemon gone.
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| unsafe {
        atexit(stop_shared_daemon_on_exit);
    });
    TestHome { path, _owned: None }
});

extern "C" {
    fn atexit(cb: extern "C" fn()) -> core::ffi::c_int;
}

extern "C" fn stop_shared_daemon_on_exit() {
    let _ = SHARED_HOME
        .command(&["proxy", "stop"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

/// A disposable HOME for spawned `antra` processes.
pub struct TestHome {
    path: PathBuf,
    // Held so a private TestHome is removed on drop; a shared one is leaked
    // on purpose (see module docs) and stores None here.
    _owned: Option<TempDir>,
}

impl TestHome {
    /// Fresh, exclusively-owned home, removed when dropped.
    pub fn new() -> Self {
        // Under `base_root`, not the per-user temp dir: see the note there
        // about the daemon's 104-byte socket path limit.
        let dir = tempfile::Builder::new()
            .prefix("h")
            .tempdir_in(base_root())
            .expect("temp dir");
        let path = dir.path().to_path_buf();
        create_layout(&path);
        Self {
            path,
            _owned: Some(dir),
        }
    }

    /// The one home shared by every test in this binary. See module docs.
    pub fn shared() -> &'static TestHome {
        &SHARED_HOME
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// A fully wired `Command`: hermetic env, plus piped stdio for the
    /// callers that need to read output or write to stdin.
    pub fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(antra_bin());
        cmd.args(args);
        for (key, value) in hermetic_env(&self.path) {
            cmd.env(key, value);
        }
        cmd
    }

    /// Where the spawned binary will keep its Antra state.
    ///
    /// Mirrors what `dirs::config_dir()` resolves to under [`hermetic_env`],
    /// spelled out because the test crate cannot depend on `dirs` directly.
    pub fn config_dir(&self) -> PathBuf {
        if cfg!(target_os = "windows") {
            self.path.join("AppData/Roaming/antra")
        } else if cfg!(target_os = "macos") {
            self.path.join("Library/Application Support/antra")
        } else {
            self.path.join(".config/antra")
        }
    }

    /// Where the daemon will put its IPC socket (unix only).
    ///
    /// macOS has no XDG runtime dir, so the daemon falls back to its config
    /// dir. Kept as a method so the length assertion in the tests below is
    /// about the real path rather than a reconstruction of it.
    #[cfg(unix)]
    pub fn socket_path(&self) -> PathBuf {
        self.config_dir().join("daemon.sock")
    }
}

impl Default for TestHome {
    fn default() -> Self {
        Self::new()
    }
}

/// Path to the `antra` binary built next to the test executable.
pub fn antra_bin() -> String {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.push("antra");
    #[cfg(target_os = "windows")]
    path.set_extension("exe");
    path.to_string_lossy().to_string()
}

/// Env overrides that redirect config, data-local and runtime/socket paths
/// into `dir` (what `dirs` reads on each platform).
fn hermetic_env(dir: &Path) -> Vec<(&'static str, std::ffi::OsString)> {
    let d = dir.to_path_buf();
    vec![
        ("HOME", d.clone().into_os_string()),
        ("XDG_CONFIG_HOME", d.join(".config").into_os_string()),
        ("XDG_DATA_HOME", d.join(".local/share").into_os_string()),
        ("XDG_RUNTIME_DIR", d.join("run").into_os_string()),
        ("TMPDIR", d.join("tmp").into_os_string()),
        ("APPDATA", d.join("AppData/Roaming").into_os_string()),
        ("LOCALAPPDATA", d.join("AppData/Local").into_os_string()),
    ]
}

/// Pre-create the directories the env map points at. `dirs` only needs the
/// parent to exist, but pre-creating them makes a misconfigured redirect
/// obvious as a permission error instead of a silent fallback to `.`.
fn create_layout(dir: &Path) {
    for sub in [
        ".config",
        ".local/share",
        "run",
        "tmp",
        "AppData/Roaming",
        "AppData/Local",
    ] {
        let _ = std::fs::create_dir_all(dir.join(sub));
    }
}

fn user_tag() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "shared".to_string())
}

/// Spawn `antra` with stdio piped and a null stdin (the default for the
/// suites: the CLI skips consent prompts when stdin is not a terminal).
pub fn spawn_antra(home: &TestHome, args: &[&str]) -> Child {
    home.command(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute antra")
}

pub fn run_antra(home: &TestHome, args: &[&str]) -> (String, String, i32) {
    run_antra_with_timeout(home, args, Duration::from_secs(10))
}

pub fn run_antra_with_timeout(
    home: &TestHome,
    args: &[&str],
    timeout: Duration,
) -> (String, String, i32) {
    collect(spawn_antra(home, args), timeout)
}

pub fn run_antra_with_dir(
    home: &TestHome,
    dir: &std::path::Path,
    args: &[&str],
) -> (String, String, i32) {
    run_antra_with_dir_timeout(home, dir, args, Duration::from_secs(10))
}

pub fn run_antra_with_dir_timeout(
    home: &TestHome,
    dir: &std::path::Path,
    args: &[&str],
    timeout: Duration,
) -> (String, String, i32) {
    let child = home
        .command(args)
        .current_dir(dir)
        // Null stdin here too: these spawns used to inherit the developer's
        // terminal, and a fresh (untrusted) home would otherwise stop on the
        // CA consent prompt.
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to execute antra");
    collect(child, timeout)
}

fn collect(mut child: Child, timeout: Duration) -> (String, String, i32) {
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().unwrap();
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return (stdout, stderr, status.code().unwrap_or(-1));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    // Don't call wait() — on Windows it hangs when the killed
                    // process still holds pipe handles or children.
                    let _ = child.kill();
                    return (String::new(), "timeout".to_string(), -1);
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return (String::new(), format!("{e}"), -1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hermetic_env_redirects_every_dirs_path() {
        let home = TestHome::new();
        let env: std::collections::HashMap<_, _> = hermetic_env(home.path()).into_iter().collect();
        for key in [
            "HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_RUNTIME_DIR",
        ] {
            let value = env.get(key).expect(key);
            assert!(
                Path::new(value).starts_with(home.path()),
                "{key} must point inside the disposable home"
            );
        }
        assert_eq!(env.get("HOME").unwrap(), home.path().as_os_str());
    }

    #[test]
    fn hermetic_layout_is_writable() {
        let home = TestHome::new();
        for (key, value) in hermetic_env(home.path()) {
            let dir = PathBuf::from(value);
            assert!(dir.is_dir(), "{key} points at a missing directory: {dir:?}");
        }
    }

    /// A unix socket path is capped at 104 bytes (`sun_path`). The hermetic
    /// home has to leave room for `$HOME/Library/Application Support/antra/
    /// daemon.sock` on macOS, where there is no XDG runtime dir to shorten
    /// it. Getting this wrong fails as `path must be shorter than SUN_LEN`
    /// from the daemon, which reads like a product bug.
    #[cfg(unix)]
    #[test]
    fn shared_home_socket_path_fits_sun_path_max() {
        const SUN_PATH_MAX: usize = 104;
        for home in [TestHome::shared(), &TestHome::new()] {
            let path = home.socket_path();
            assert!(
                path.as_os_str().len() < SUN_PATH_MAX,
                "socket path is {} bytes, over the {SUN_PATH_MAX}-byte limit: {}",
                path.as_os_str().len(),
                path.display()
            );
        }
    }

    #[test]
    fn shared_home_is_stable_within_a_process() {
        let a = TestHome::shared().path().to_path_buf();
        let b = TestHome::shared().path().to_path_buf();
        assert_eq!(a, b);
        assert!(a.is_dir());
    }
}
