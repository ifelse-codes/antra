use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A persisted domain→port mapping.
///
/// - Static aliases (`antra alias` / `antra add route`): `managed=false`,
///   `pid=None` — long-lived, always restored.
/// - Managed routes (`antra run` / `dev`): `managed=true`, `pid=Some(..)` —
///   restored only when the recorded PID is still alive; stale entries are
///   dropped on daemon start.
///
/// `pid`/`managed` default for backward compat: `aliases.json` files written
/// before managed persistence contain only `{domain, port}` and deserialize
/// as static aliases.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AliasEntry {
    pub domain: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default)]
    pub managed: bool,
}

fn aliases_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("antra").join("aliases.json"))
}

/// Load persisted aliases. Missing or corrupt files yield an empty list —
/// a broken cache must never prevent the daemon from starting.
pub fn load_aliases() -> Vec<AliasEntry> {
    let Some(path) = aliases_path() else {
        return Vec::new();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

/// Persist aliases. Best-effort: failures are ignored so a read-only or
/// full disk never breaks route registration. An empty list removes the
/// file instead of writing `[]`.
pub fn save_aliases(entries: &[AliasEntry]) {
    let Some(path) = aliases_path() else {
        return;
    };
    if entries.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if let Ok(json) = serde_json::to_string_pretty(entries) {
        let _ = std::fs::write(&path, json);
        // sudo-root daemon writes into the user's config dir: hand the file
        // back so later unprivileged commands can update it.
        #[cfg(unix)]
        crate::platform::chown_to_invoking_user(&path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alias_entries_round_trip_through_json() {
        // In-memory format check only — never touches the real state dir.
        let entries = vec![
            AliasEntry {
                domain: "a.localhost".to_string(),
                port: 1000,
                pid: None,
                managed: false,
            },
            AliasEntry {
                domain: "b.localhost".to_string(),
                port: 2000,
                pid: Some(1234),
                managed: true,
            },
        ];
        let json = serde_json::to_string_pretty(&entries).unwrap();
        let back: Vec<AliasEntry> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].domain, "a.localhost");
        assert_eq!(back[1].port, 2000);
        assert_eq!(back[1].pid, Some(1234));
        assert!(back[1].managed);
        // Legacy files with only {domain, port} decode as static aliases.
        let legacy: Vec<AliasEntry> =
            serde_json::from_str(r#"[{"domain":"old.localhost","port":3000}]"#).unwrap();
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].pid, None);
        assert!(!legacy[0].managed);
        // Corrupt input must yield empty, never panic (daemon must start).
        let corrupt: Vec<AliasEntry> = serde_json::from_str("not json{[}").unwrap_or_default();
        assert!(corrupt.is_empty());
    }
}
