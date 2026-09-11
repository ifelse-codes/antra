use std::collections::HashMap;
use std::sync::RwLock;

use crate::routing::persist::AliasEntry;
use crate::routing::types::Route;

pub struct RouteRegistry {
    routes: RwLock<HashMap<String, Route>>,
    /// Persist unmanaged (static alias) routes to disk so they survive
    /// daemon restarts. Disabled for ephemeral (test) registries so unit
    /// tests never touch the user's real state directory.
    persist: bool,
}

/// Snapshot routes to disk so they survive daemon restarts.
///
/// - Static aliases (`managed=false`) are always persisted.
/// - Managed `run`/`dev` routes (`managed=true`) persist only with a real
///   child PID so a restart can keep them when the owner is still alive and
///   drop them when it is gone. Pid-less managed routes are zombies from a
///   pre-spawn registration and must never reach disk.
///
/// Hot path (`lookup`/`list`) never touches disk; only register/unregister
/// write, and restore reads once at daemon start.
fn persist_snapshot(routes: &HashMap<String, Route>) -> Vec<AliasEntry> {
    routes
        .values()
        .filter(|r| !r.managed || r.pid.is_some())
        .map(|r| AliasEntry {
            domain: r.domain.clone(),
            port: r.port,
            pid: r.pid,
            managed: r.managed,
        })
        .collect()
}

impl RouteRegistry {
    /// Production registry: static aliases are persisted to disk.
    pub fn new() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            persist: true,
        }
    }

    /// Non-persisting registry for tests and throwaway use.
    /// (Only exercised by integration tests, hence the allow.)
    #[allow(dead_code)]
    pub fn new_ephemeral() -> Self {
        Self {
            routes: RwLock::new(HashMap::new()),
            persist: false,
        }
    }

    /// Write the snapshot unless this is an ephemeral registry.
    fn maybe_persist(&self, snapshot: &[AliasEntry]) {
        if self.persist {
            crate::routing::persist::save_aliases(snapshot);
        }
    }

    pub fn register(&self, route: Route) -> anyhow::Result<()> {
        // Snapshot under the lock, persist after it's released.
        let snapshot = {
            let mut routes = self
                .routes
                .write()
                .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
            routes.insert(route.domain.clone(), route);
            persist_snapshot(&routes)
        };
        self.maybe_persist(&snapshot);
        Ok(())
    }

    pub fn unregister(&self, domain: &str) -> anyhow::Result<()> {
        let snapshot = {
            let mut routes = self
                .routes
                .write()
                .map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
            routes.remove(domain);
            persist_snapshot(&routes)
        };
        self.maybe_persist(&snapshot);
        Ok(())
    }

    pub fn lookup(&self, domain: &str) -> Option<Route> {
        let routes = self.routes.read().ok()?;
        routes.get(domain).cloned()
    }

    pub fn list(&self) -> Vec<Route> {
        let routes = self.routes.read().unwrap_or_else(|e| e.into_inner());
        routes.values().cloned().collect()
    }
}

impl Default for RouteRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routing::types::Protocol;
    use std::net::{IpAddr, Ipv4Addr};
    use std::time::Instant;

    fn route(domain: &str, port: u16, pid: Option<u32>, managed: bool) -> Route {
        Route {
            domain: domain.to_string(),
            host: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            pid,
            managed,
            protocol: Protocol::Http,
            created_at: Instant::now(),
        }
    }

    #[test]
    fn persist_snapshot_keeps_static_and_managed_with_pid() {
        let mut map = HashMap::new();
        map.insert(
            "static.localhost".to_string(),
            route("static.localhost", 3000, None, false),
        );
        map.insert(
            "run.localhost".to_string(),
            route("run.localhost", 5173, Some(1234), true),
        );
        let snap = persist_snapshot(&map);
        assert_eq!(snap.len(), 2);
        let managed = snap.iter().find(|e| e.domain == "run.localhost").unwrap();
        assert!(managed.managed);
        assert_eq!(managed.pid, Some(1234));
        let statics: Vec<_> = snap.iter().filter(|e| !e.managed).collect();
        assert_eq!(statics.len(), 1);
        assert_eq!(statics[0].domain, "static.localhost");
    }

    #[test]
    fn managed_route_without_pid_still_excluded() {
        // Defensive: even a pid-less managed route must never reach disk.
        let mut map = HashMap::new();
        map.insert(
            "orphan.localhost".to_string(),
            route("orphan.localhost", 4000, None, true),
        );
        assert!(persist_snapshot(&map).is_empty());
    }
}
