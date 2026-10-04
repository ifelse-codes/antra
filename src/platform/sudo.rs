//! Acting for the user who ran `sudo` (ROADMAP C27).
//!
//! Port 443 needs root on macOS and Linux, so serving `https://app.localhost`
//! with no port number means starting the daemon under `sudo`. Two things
//! make that work and keep it safe:
//!
//! * **Paths.** `sudo` on Linux resets `HOME` to root's and drops
//!   `XDG_RUNTIME_DIR`, so a daemon started by `sudo antra proxy start` put
//!   its socket and CA under `/root`, where the user's own CLI never looks —
//!   it reported "not running" and started a second daemon on 8443.
//!   [`adopt_invoking_user_paths`] puts the user's paths back. macOS `sudo`
//!   keeps `HOME`, so there it changes nothing.
//! * **Privileges.** A root daemon parses TLS from anything that connects to
//!   it, and writes certificates into a directory the user controls.
//!   [`bind_then_drop`] opens the ports as root and then becomes the user for
//!   good, before the daemon reads or writes a single file.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

use crate::daemon::server::Prebound;

/// The user who ran `sudo`, when this process is root because of it.
pub struct InvokingUser {
    pub uid: u32,
    pub gid: u32,
    pub name: String,
    pub home: PathBuf,
}

/// `Some` only when this process is root *and* `sudo` says who asked for it.
/// Plain root (a container, a root login) has no one to hand back to, and
/// keeps behaving as it always did.
pub fn invoking_user() -> Option<InvokingUser> {
    // SAFETY: geteuid cannot fail and has no preconditions.
    if unsafe { libc::geteuid() } != 0 {
        return None;
    }
    let uid: u32 = std::env::var("SUDO_UID").ok()?.parse().ok()?;
    if uid == 0 {
        return None;
    }
    let user = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(uid)).ok()??;
    let gid = std::env::var("SUDO_GID")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(user.gid.as_raw());
    Some(InvokingUser {
        uid,
        gid,
        name: user.name,
        home: user.dir,
    })
}

/// Point `HOME` and `XDG_RUNTIME_DIR` back at the invoking user's, so this
/// process finds the same socket, CA and log as the user's CLI.
///
/// Sets environment variables, so it must run before any thread is spawned:
/// `antra proxy` calls it first thing, before the tokio runtime exists.
pub fn adopt_invoking_user_paths(user: &InvokingUser) {
    let root_home = nix::unistd::User::from_uid(nix::unistd::Uid::from_raw(0))
        .ok()
        .flatten()
        .map(|u| u.dir);
    let changes = paths_to_adopt(
        std::env::var_os("HOME").as_deref(),
        std::env::var_os("XDG_RUNTIME_DIR").as_deref(),
        root_home.as_deref(),
        user,
        user_runtime_dir(user.uid),
    );
    for (var, value) in changes {
        tracing::debug!(var, value = ?value, "Using the invoking user's path under sudo");
        std::env::set_var(var, value);
    }
}

/// What [`adopt_invoking_user_paths`] sets, as a pure function of what it
/// finds. A value the caller passed on purpose — through `sudo -E`, `sudo env
/// HOME=…`, or macOS keeping `HOME` — is kept; only root's own is replaced.
fn paths_to_adopt(
    home: Option<&OsStr>,
    runtime_dir: Option<&OsStr>,
    root_home: Option<&Path>,
    user: &InvokingUser,
    user_runtime: Option<PathBuf>,
) -> Vec<(&'static str, OsString)> {
    let mut changes = Vec::new();

    let home_is_roots = match home {
        None => true,
        Some(h) => {
            let h = Path::new(h);
            root_home == Some(h) || h == Path::new("/root")
        }
    };
    if home_is_roots {
        changes.push(("HOME", user.home.clone().into_os_string()));
    }

    let runtime_is_roots = match runtime_dir {
        None => true,
        Some(dir) => Path::new(dir) == Path::new("/run/user/0"),
    };
    if runtime_is_roots {
        if let Some(dir) = user_runtime {
            changes.push(("XDG_RUNTIME_DIR", dir.into_os_string()));
        }
    }

    changes
}

/// The user's `XDG_RUNTIME_DIR` as systemd-logind creates it, if it exists
/// and is theirs. `None` on macOS and on Linux without logind, where the
/// user's own CLI has no runtime dir either and both fall back to `HOME`.
fn user_runtime_dir(uid: u32) -> Option<PathBuf> {
    use std::os::unix::fs::MetadataExt;
    let dir = PathBuf::from(format!("/run/user/{uid}"));
    let meta = std::fs::metadata(&dir).ok()?;
    (meta.is_dir() && meta.uid() == uid).then_some(dir)
}

/// Open the daemon's ports as root, then become `user` for good.
///
/// A port that cannot be opened here (taken by something else) is left out;
/// the daemon then tries it again as the user and falls back as usual. Fails
/// only if root cannot be given up, and then the daemon must not start.
pub fn bind_then_drop(user: &InvokingUser, https_port: u16, http_port: u16) -> Result<Prebound> {
    let https = bind_loopback_std(https_port).ok().map(|l| (https_port, l));
    let http = bind_loopback_std(http_port).ok().map(|l| (http_port, l));
    drop_privileges(user)?;
    tracing::info!(
        uid = user.uid,
        https = https.is_some(),
        http = http.is_some(),
        "Opened the ports as root, now running as the invoking user"
    );
    Ok(Prebound { https, http })
}

fn bind_loopback_std(port: u16) -> std::io::Result<Vec<std::net::TcpListener>> {
    let v4 = std::net::TcpListener::bind(("127.0.0.1", port));
    let v6 = std::net::TcpListener::bind(("::1", port));
    crate::util::port::loopback_binds(v4, v6)
}

fn drop_privileges(user: &InvokingUser) -> Result<()> {
    let name = std::ffi::CString::new(user.name.as_bytes())?;
    // SAFETY: plain libc calls on values we own; each result is checked.
    // Groups first, then gid, then uid: after setuid nothing else is allowed.
    unsafe {
        if libc::initgroups(name.as_ptr(), user.gid as _) != 0 {
            bail!("initgroups failed: {}", std::io::Error::last_os_error());
        }
        if libc::setgid(user.gid as libc::gid_t) != 0 {
            bail!("setgid failed: {}", std::io::Error::last_os_error());
        }
        if libc::setuid(user.uid as libc::uid_t) != 0 {
            bail!("setuid failed: {}", std::io::Error::last_os_error());
        }
        // As root, setuid sets the real, effective and saved uid. Prove it:
        // if root can still be had back, refuse to run at all.
        if libc::geteuid() != user.uid || libc::getuid() != user.uid || libc::setuid(0) == 0 {
            bail!("still able to regain root after dropping privileges");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alice() -> InvokingUser {
        InvokingUser {
            uid: 1000,
            gid: 1000,
            name: "alice".into(),
            home: PathBuf::from("/home/alice"),
        }
    }

    fn adopt(
        home: Option<&str>,
        runtime: Option<&str>,
        user_runtime: Option<&str>,
    ) -> Vec<(&'static str, OsString)> {
        paths_to_adopt(
            home.map(OsStr::new),
            runtime.map(OsStr::new),
            Some(Path::new("/root")),
            &alice(),
            user_runtime.map(PathBuf::from),
        )
    }

    #[test]
    fn linux_sudo_gets_the_users_home_and_runtime_dir_back() {
        // What `sudo` leaves on Linux: HOME is root's, XDG_RUNTIME_DIR gone.
        assert_eq!(
            adopt(Some("/root"), None, Some("/run/user/1000")),
            vec![
                ("HOME", OsString::from("/home/alice")),
                ("XDG_RUNTIME_DIR", OsString::from("/run/user/1000")),
            ]
        );
    }

    #[test]
    fn a_home_passed_on_purpose_is_kept() {
        // macOS sudo keeps HOME; `sudo env HOME=…` and `sudo -E` pass one.
        // A hermetic test HOME is the same case.
        assert!(adopt(Some("/Users/alice"), None, None).is_empty());
        assert!(adopt(
            Some("/tmp/ah-test"),
            Some("/tmp/rt"),
            Some("/run/user/1000")
        )
        .is_empty());
    }

    #[test]
    fn roots_runtime_dir_is_replaced_and_a_missing_one_is_not_invented() {
        assert_eq!(
            adopt(
                Some("/home/alice"),
                Some("/run/user/0"),
                Some("/run/user/1000")
            ),
            vec![("XDG_RUNTIME_DIR", OsString::from("/run/user/1000"))]
        );
        // No logind dir for the user: their own CLI has none either.
        assert!(adopt(Some("/home/alice"), None, None).is_empty());
    }

    #[test]
    fn roots_home_is_recognised_where_it_is_not_slash_root() {
        let changes = paths_to_adopt(
            Some(OsStr::new("/var/root")),
            Some(OsStr::new("/tmp/rt")),
            Some(Path::new("/var/root")),
            &alice(),
            None,
        );
        assert_eq!(changes, vec![("HOME", OsString::from("/home/alice"))]);
    }

    #[test]
    fn no_home_at_all_gets_the_users() {
        assert_eq!(
            adopt(None, Some("/tmp/rt"), None),
            vec![("HOME", OsString::from("/home/alice"))]
        );
    }
}
