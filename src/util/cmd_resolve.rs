use std::path::{Path, PathBuf};

/// Resolve a program name to an executable path on Windows.
///
/// `tokio::process::Command::new("npm")` does no PATH / extension lookup
/// and the daemon's env differs from the user's shell, so `antra run --
/// npm ...` failed with "program not found" unless the full
/// `C:\Program Files\nodejs\npm.cmd` path was given.
///
/// Resolution order:
/// 1. Direct path that exists (as-is, e.g. full path or `./foo`).
/// 2. Each dir in `PATH` env, probing executable extensions first on Windows
///    (`program.cmd`, `program.exe`, `program.bat`, then bare `program`).
/// 3. Well-known Windows Node.js locations:
///    `C:\Program Files\nodejs\`, `%APPDATA%\npm\`,
///    `%LOCALAPPDATA%\Programs\nodejs\` (same extension probes).
///
/// On Unix the extension probes are skipped; a bare name found in `PATH`
/// returns the full path, otherwise the input is returned unchanged so the
/// spawn error still shows what was requested.
pub fn resolve_program(program: &str) -> PathBuf {
    let direct = Path::new(program);
    if direct.exists() {
        return direct.to_path_buf();
    }
    // A path with a separator that doesn't exist: return as-is so the
    // caller reports the exact path the user gave.
    //
    // Windows order matters: `C:\Program Files\nodejs\npm` (extensionless
    // POSIX shell script) exists alongside `npm.cmd` but fails with OS error
    // 193 ("%1 is not a valid Win32 application"). Probe executable
    // extensions first so `npm` resolves to `npm.cmd`.
    #[cfg(windows)]
    const EXTS: &[&str] = &[".cmd", ".exe", ".bat", ""];
    #[cfg(not(windows))]
    const EXTS: &[&str] = &[""];

    // 2. Search PATH.
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            for ext in EXTS {
                let candidate = dir.join(format!("{program}{ext}"));
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }

    // 3. Well-known Windows Node.js locations.
    #[cfg(windows)]
    {
        let mut extra_dirs: Vec<PathBuf> = Vec::new();
        extra_dirs.push(PathBuf::from(r"C:\Program Files\nodejs"));
        extra_dirs.push(PathBuf::from(r"C:\Program Files (x86)\nodejs"));
        if let Some(appdata) = std::env::var_os("APPDATA") {
            extra_dirs.push(PathBuf::from(appdata).join("npm"));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            extra_dirs.push(PathBuf::from(local).join(r"Programs\nodejs"));
        }
        for dir in extra_dirs {
            for ext in EXTS {
                let candidate = dir.join(format!("{program}{ext}"));
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
    }

    // Not found: return input unchanged; the spawn error path reports it.
    PathBuf::from(program)
}

/// Hint shown when a spawn fails, pointing Windows users at the fix.
pub fn spawn_hint(program: &str, resolved: &Path) -> String {
    #[cfg(windows)]
    {
        format!(
            "Resolved '{program}' to '{}'. Ensure Node.js is installed and on PATH (e.g. C:\\Program Files\\nodejs), or pass the full path to npm.cmd.",
            resolved.display()
        )
    }
    #[cfg(not(windows))]
    {
        let _ = resolved;
        format!("Ensure '{program}' is installed and on PATH for the daemon environment.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_path_returned_as_is() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("myprog");
        std::fs::write(&exe, "x").unwrap();
        assert_eq!(resolve_program(&exe.to_string_lossy()), exe);
    }

    /// Process-global `PATH` is mutated below: serialize against parallel
    /// tests touching the environment and restore on drop so a panicking
    /// assert cannot leak the temp dir into other tests' lookups.
    static PATH_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct RestorePath {
        old: Option<std::ffi::OsString>,
    }

    impl Drop for RestorePath {
        fn drop(&mut self) {
            match self.old.take() {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
        }
    }

    #[test]
    fn finds_program_in_path() {
        let _lock = PATH_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _restore = RestorePath {
            old: std::env::var_os("PATH"),
        };
        let dir = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let name = "antra-test-prog.cmd";
        #[cfg(not(windows))]
        let name = "antra-test-prog";
        std::fs::write(dir.path().join(name), "x").unwrap();
        let mut paths = vec![dir.path().to_path_buf()];
        if let Some(p) = &_restore.old {
            paths.extend(std::env::split_paths(p));
        }
        let joined = std::env::join_paths(paths).unwrap();
        std::env::set_var("PATH", &joined);
        let stem = name.trim_end_matches(".cmd");
        // On Windows both bare stem and full name must resolve.
        let resolved = resolve_program(stem);
        assert!(
            resolved.is_file(),
            "expected {stem} to resolve, got {resolved:?}"
        );
    }

    #[test]
    fn missing_program_returns_input() {
        let p = resolve_program("antra-definitely-missing-binary-xyz");
        assert_eq!(p, PathBuf::from("antra-definitely-missing-binary-xyz"));
    }
}
