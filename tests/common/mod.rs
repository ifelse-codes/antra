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

/// A tag that is stable within a worktree and distinct between worktrees.
///
/// The disposable homes under `base_root()` used to be shared by every
/// worktree on the machine, which is only safe while one `cargo test` runs at a
/// time. It does not hold: `cargo test` is also how an agent session verifies
/// its work, so several worktrees reach this code concurrently, and the
/// failure is silent and spectacular.
///
/// The daemon writes a CA certificate and its private key as two separate
/// files. Two runs interleaving between those writes leave a `ca.pem` whose
/// public key does not match `ca-key.pem`. Every leaf signed afterwards then
/// fails verification, and `tests/e2e_securetransport.rs` surfaces it as
/// `LibreSSL ... asn1 encoding routines:CRYPTO_internal:EVP lib` — which reads
/// as a certificate-encoding bug in the product and is nothing of the kind.
/// Wiping `/tmp/antra-e2e` made it vanish, which is what gave it away.
///
/// A readable prefix keeps `ls /tmp/antra-e2e` diagnosable; the hash suffix
/// keeps two worktrees whose directory names share a long prefix apart, which
/// a plain truncated name would not.
fn worktree_tag() -> String {
    let raw = std::env::current_dir()
        .ok()
        .and_then(|d| d.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "root".to_string());
    worktree_tag_for(&raw)
}

/// The tagging itself, separated so the collision property is testable:
/// `worktree_tag()` reads process-global state that a parallel test cannot
/// change, but the two checkouts it must keep apart are just strings.
fn worktree_tag_for(raw: &str) -> String {
    let mut prefix: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .take(12)
        .collect();
    if prefix.is_empty() {
        prefix.push_str("wt");
    }

    // FNV-1a over the full name, not the prefix: two checkouts can differ only
    // past the twelfth character, and that is exactly the collision a truncated
    // name reintroduces.
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x1000_0000_01b3;
    let mut hash = OFFSET;
    for byte in raw.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }

    format!("{prefix}-{:06x}", hash & 0xff_ffff)
}

/// The one home shared by every test in a binary. See module docs: the
/// daemon has a single fallback port (443 → 8443, `daemon/server.rs`), so a
/// second home per suite would mean a second daemon that cannot bind and
/// bails. The user name keeps the path stable across the e2e binaries,
/// which `cargo test` runs sequentially; `worktree_tag` keeps it stable
/// across binaries but distinct between worktrees running at the same time.
static SHARED_HOME: LazyLock<TestHome> = LazyLock::new(|| {
    let path = base_root().join(user_tag()).join(worktree_tag());
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

    /// Where the daemon writes its log, mirroring what
    /// `util::logs::daemon_log_path()` resolves through `data_local_dir()`.
    ///
    /// Deliberately *not* derived from [`Self::config_dir`]: on Linux config
    /// is `$XDG_CONFIG_HOME` and data-local is `$XDG_DATA_HOME`, so a test that
    /// guessed from the config dir passed on macOS (where both land under
    /// `Library/Application Support`) and failed on Ubuntu.
    pub fn log_dir(&self) -> PathBuf {
        if cfg!(target_os = "windows") {
            self.path.join("AppData/Local/antra")
        } else if cfg!(target_os = "macos") {
            self.path.join("Library/Application Support/antra")
        } else {
            self.path.join(".local/share/antra")
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

    /// The worktree tag is what keeps two checkouts from sharing a disposable
    /// home — and therefore from interleaving writes to `ca.pem` /
    /// `ca-key.pem`. Assert both halves: stable within a process, and built
    /// from the checkout rather than the user name alone.
    #[test]
    fn worktree_tag_is_stable_and_namespaces_the_shared_home() {
        assert_eq!(worktree_tag(), worktree_tag(), "tag must be stable");

        let tag = worktree_tag();
        assert!(
            tag.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
            "tag must be filesystem-safe: {tag}"
        );
        // Bounded: the socket path budget is 104 bytes and this sits in it.
        assert!(tag.len() <= 20, "tag is {} bytes: {tag}", tag.len());

        let home = TestHome::shared().path().to_path_buf();
        assert!(
            home.components()
                .any(|c| c.as_os_str() == std::ffi::OsStr::new(&tag)),
            "shared home {} must sit under its worktree tag {tag}",
            home.display()
        );
    }

    /// Two checkouts whose names differ only past the twelfth character must
    /// not collide — a plain truncated name would, and that collision is the
    /// bug this whole change exists to prevent.
    #[test]
    fn worktree_tag_separates_checkouts_sharing_a_prefix() {
        let a = worktree_tag_for(&format!("{}-alpha", "shared-prefix"));
        let b = worktree_tag_for(&format!("{}-omega", "shared-prefix"));
        assert_ne!(
            a, b,
            "checkouts differing only past the truncation point must not collide"
        );
        // Identical input, identical tag: stability is the other half.
        assert_eq!(a, worktree_tag_for(&format!("{}-alpha", "shared-prefix")));
    }

    /// The tag lands in a 104-byte socket path, so its length is a budget, not
    /// a style choice. Anything unsafe for a path also has to be sanitised.
    #[test]
    fn worktree_tag_is_bounded_and_filesystem_safe() {
        for raw in [
            "antra",
            "antra-3",
            "a-very-long-checkout-name-that-keeps-going-and-going",
            "weird name/with:separators",
            "",
        ] {
            let tag = worktree_tag_for(raw);
            assert!(!tag.is_empty(), "tag must not be empty for {raw:?}");
            assert!(
                tag.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
                "tag must be filesystem-safe: {tag} (from {raw:?})"
            );
            assert!(
                tag.len() <= 20,
                "tag is {} bytes for {raw:?}: {tag}",
                tag.len()
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
